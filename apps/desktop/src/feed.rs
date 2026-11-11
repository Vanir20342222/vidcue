//! Feed pages: first pages, continuations, radio offsets and retries.
//!
//! Each route keeps its own page so back/forward is instant. Retryable provider failures back
//! off 2 s, 4 s, 8 s as the Android client does; account failures offer sign-in instead.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::json;
use vidcue_core::{BrowseRequest, Category, Media, Page};

use crate::app::{App, FeedRoute, Tab};
use crate::backend::{ApiError, op};

const STALE_AFTER: f64 = 15.0 * 60.0;

#[derive(Default)]
pub struct FeedState {
    pub title: String,
    pub items: Vec<Media>,
    pub categories: Vec<Category>,
    pub continuation: Option<String>,
    pub notice: Option<String>,
    pub loading: bool,
    pub loading_more: bool,
    pub error: Option<ApiError>,
    pub attempts: u32,
    pub fetched_at: Option<f64>,
    /// Radio pages continue by offset; this is false once a page came back short.
    pub radio_more: bool,
    pub generation: u64,
}

impl FeedState {
    pub fn loaded(&self) -> bool {
        self.fetched_at.is_some()
    }
}

#[derive(Default)]
pub struct Feeds {
    pub pages: HashMap<String, FeedState>,
    pub seq: u64,
}

impl Feeds {
    pub fn get(&self, route: &FeedRoute) -> Option<&FeedState> {
        self.pages.get(&route.key())
    }
}

/// Loads `route` unless a fresh page is already there (or `force`).
pub fn ensure(app: &mut App, route: &FeedRoute, force: bool) {
    let key = route.key();
    let now = app.clock;
    let offline = app.offline();
    app.feeds.seq += 1;
    let generation = app.feeds.seq;
    let state = app.feeds.pages.entry(key.clone()).or_default();
    if state.loading {
        return;
    }
    let fresh = state.fetched_at.is_some_and(|t| now - t < STALE_AFTER);
    if fresh && !force {
        return;
    }
    if !force {
        state.attempts = 0;
    }
    state.loading = true;
    state.error = None;
    state.generation = generation;
    if offline {
        state.loading = false;
        return;
    }
    let route = route.clone();
    app.be.call(
        op(
            "browse",
            json!({ "request": route.request, "tab": route.tab.id() }),
        ),
        move |app, r, tx| {
            let clock = app.clock;
            let be = app.be.clone();
            let Some(state) = app.feeds.pages.get_mut(&key) else {
                return;
            };
            if state.generation != generation {
                return;
            }
            state.loading = false;
            match r.and_then(|v| {
                serde_json::from_value::<Page>(v).map_err(|e| ApiError {
                    code: "provider_error",
                    message: e.to_string(),
                    retry_after_ms: None,
                })
            }) {
                Ok(page) => {
                    state.radio_more = route.tab == Tab::Radio
                        && !page.items.is_empty()
                        && page.continuation.is_none();
                    state.title = page.title;
                    state.items = page.items;
                    state.categories = page.categories;
                    state.continuation = page.continuation;
                    state.notice = page.notice;
                    state.fetched_at = Some(clock);
                    state.attempts = 0;
                }
                Err(e) => {
                    if e.retryable() && state.attempts < 3 {
                        state.attempts += 1;
                        let wait = e.retry_after_ms.unwrap_or(0).max(1000 << state.attempts);
                        let route = route.clone();
                        state.error = Some(ApiError {
                            message: format!("{} Retrying…", e.message),
                            ..e
                        });
                        be.task(
                            tokio::time::sleep(Duration::from_millis(wait)),
                            move |app, _, _| ensure(app, &route, true),
                        );
                    } else {
                        state.error = Some(e);
                    }
                    let _ = tx;
                }
            }
        },
    );
}

/// Loads the next page of `route`, if it has one.
pub fn more(app: &mut App, route: &FeedRoute) {
    let key = route.key();
    let Some(state) = app.feeds.pages.get_mut(&key) else {
        return;
    };
    if state.loading || state.loading_more {
        return;
    }
    let (request, offset) = if let Some(token) = state.continuation.clone() {
        (BrowseRequest::Continue(token), None)
    } else if state.radio_more {
        (route.request.clone(), Some(state.items.len() as u64))
    } else {
        return;
    };
    state.loading_more = true;
    let generation = state.generation;
    let mut req = json!({ "request": request, "tab": route.tab.id() });
    if let Some(o) = offset {
        req["offset"] = json!(o);
    }
    app.be.call(op("browse", req), move |app, r, _| {
        let Some(state) = app.feeds.pages.get_mut(&key) else {
            return;
        };
        if state.generation != generation {
            return;
        }
        state.loading_more = false;
        match r.and_then(|v| {
            serde_json::from_value::<Page>(v).map_err(|e| ApiError {
                code: "provider_error",
                message: e.to_string(),
                retry_after_ms: None,
            })
        }) {
            Ok(page) => {
                let before = state.items.len();
                let known: std::collections::HashSet<String> =
                    state.items.iter().map(|m| m.url.clone()).collect();
                state
                    .items
                    .extend(page.items.into_iter().filter(|m| !known.contains(&m.url)));
                state.continuation = page.continuation;
                if offset.is_some() {
                    state.radio_more = state.items.len() > before;
                }
            }
            Err(e) => {
                state.continuation = None;
                state.radio_more = false;
                app.toast(
                    crate::app::ToastKind::Error,
                    format!("More results could not load: {e}"),
                );
            }
        }
    });
}

/// The chips shown above a root feed, as Android shows them.
pub fn chips(tab: Tab) -> Vec<(&'static str, BrowseRequest)> {
    match tab {
        Tab::Home => vec![
            ("For you", BrowseRequest::Home),
            ("Subscriptions", BrowseRequest::Subscriptions),
            ("Watch later", BrowseRequest::WatchLater),
            ("Liked", BrowseRequest::Liked),
            ("My playlists", BrowseRequest::Playlists),
            ("Library", BrowseRequest::Library),
            (
                "Saved locally",
                BrowseRequest::SavedMedia {
                    query: String::new(),
                },
            ),
        ],
        Tab::Music => vec![
            ("For you", BrowseRequest::Home),
            ("My playlists", BrowseRequest::Playlists),
            ("Liked", BrowseRequest::Liked),
            ("Library", BrowseRequest::Library),
            (
                "Saved locally",
                BrowseRequest::SavedMedia {
                    query: String::new(),
                },
            ),
        ],
        Tab::Radio => vec![
            ("Cliamp channels", BrowseRequest::Cliamp),
            ("Saved stations", BrowseRequest::Favorites),
        ],
    }
}

/// True for routes reached from the chip row, which replace each other instead of stacking.
pub fn is_chip(route: &FeedRoute) -> bool {
    chips(route.tab).iter().any(|(_, r)| *r == route.request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_keys_separate_tabs_and_requests() {
        let a = FeedRoute::root(Tab::Home);
        let b = FeedRoute::root(Tab::Music);
        let c = FeedRoute {
            tab: Tab::Home,
            request: BrowseRequest::Search("x".into()),
            title: "x".into(),
        };
        assert_ne!(a.key(), b.key());
        assert_ne!(a.key(), c.key());
        assert!(is_chip(&a));
        assert!(!is_chip(&c));
    }
}
