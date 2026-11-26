//! The Vidcue account on the desktop: sign in to a self-hosted Vidcue server (or claim a
//! pairing link from another device), see the sync state, sync now, choose what syncs, list and
//! revoke devices, and show a QR code that pairs a phone. Everything goes through the shared
//! engine's `sync_*` ops, the same ones the Android app uses.

use rune::{Color, El, TextEdit, Tx, col, el, icon, row, text};
use serde_json::{Value, json};

use crate::app::{App, ToastKind};
use crate::backend::op;
use crate::icons as ic;
use crate::theme::{sz, ty};
use crate::ui::widgets::{self, Kind, button};

/// The engine's collections, with the names the Android app gives them.
pub const COLLECTIONS: [(&str, &str); 10] = [
    ("positions", "Resume positions"),
    ("history", "History"),
    ("saved", "Saved"),
    ("playlists", "Playlists"),
    ("offline_playlists", "Offline playlists"),
    ("subscriptions", "Followed channels"),
    ("settings", "Settings"),
    ("eq_presets", "Equalizer presets"),
    ("filters", "Feed filters"),
    ("secrets", "Account secrets"),
];

pub struct SyncUi {
    pub status: Value,
    pub devices: Vec<Value>,
    pub server: TextEdit,
    pub username: TextEdit,
    pub password: TextEdit,
    pub code: TextEdit,
    pub link: TextEdit,
    /// A pairing link inspected and waiting for confirmation: (uri, origin, secure).
    pub link_check: Option<(String, String, bool)>,
    /// The QR code that pairs another device, as rows of "0"/"1".
    pub pair: Option<(Vec<String>, String)>,
    pub pair_password: TextEdit,
    pub busy: Option<&'static str>,
    pub error: Option<String>,
    pub confirm_sign_out: bool,
    /// The server's recommendations (opt-in), shown as a shelf on Home.
    pub recs: Vec<vidcue_core::Media>,
    pub recs_settings: Value,
}

impl Default for SyncUi {
    fn default() -> Self {
        Self {
            status: Value::Null,
            devices: Vec::new(),
            server: TextEdit::new("https://"),
            username: TextEdit::new(""),
            password: TextEdit::new("").secure(),
            code: TextEdit::new("").secure(),
            link: TextEdit::new(""),
            link_check: None,
            pair: None,
            pair_password: TextEdit::new("").secure(),
            busy: None,
            error: None,
            confirm_sign_out: false,
            recs: Vec::new(),
            recs_settings: Value::Null,
        }
    }
}

impl SyncUi {
    pub fn signed_in(&self) -> bool {
        self.status["signed_in"].as_bool().unwrap_or(false)
    }
}

/// This computer's name on the server's device list.
fn device_name() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .map(|h| h.trim().to_owned())
        .filter(|h| !h.is_empty());
    match host {
        Some(h) => format!("Vidcue on {}", h.chars().take(40).collect::<String>()),
        None => "Vidcue desktop".into(),
    }
}

// ---------------------------------------------------------------------------------------------
// Engine calls

pub fn enter(app: &mut App) {
    app.sync.confirm_sign_out = false;
    refresh(app);
}

pub fn refresh(app: &mut App) {
    app.be.call(op("sync_status", json!({})), |app, r, _| {
        if let Ok(v) = r {
            let signed_in = v["signed_in"].as_bool().unwrap_or(false);
            app.sync.status = v;
            if signed_in {
                refresh_devices(app);
                refresh_recs(app);
            } else {
                app.sync.devices.clear();
                app.sync.recs.clear();
            }
        }
    });
}

fn refresh_recs(app: &mut App) {
    app.be.call(op("recs_settings", json!({})), |app, r, _| {
        if let Ok(v) = r {
            let on = v["enabled"].as_bool().unwrap_or(false);
            app.sync.recs_settings = v;
            if on {
                app.be.call(op("recs", json!({})), |app, r, _| {
                    if let Ok(v) = r {
                        app.sync.recs = v["items"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|m| serde_json::from_value(m.clone()).ok())
                            .collect();
                    }
                });
            } else {
                app.sync.recs.clear();
            }
        }
    });
}

fn set_recs(app: &mut App, field: &'static str, on: bool) {
    app.be.call(
        op("recs_settings", json!({ field: on })),
        |app, r, _| match r {
            Ok(_) => refresh_recs(app),
            Err(e) => app.toast(ToastKind::Error, e.to_string()),
        },
    );
}

fn refresh_devices(app: &mut App) {
    app.be.call(op("sync_devices", json!({})), |app, r, _| {
        if let Ok(v) = r {
            app.sync.devices = v["devices"].as_array().cloned().unwrap_or_default();
        }
    });
}

/// After a sign-in, a claimed link or a sync: the library may have new playlists, follows,
/// filters, history and settings from the other devices.
fn landed(app: &mut App, v: Value) {
    app.sync.busy = None;
    app.sync.error = None;
    app.sync.status = v;
    crate::ui::library_local::refresh(app);
    crate::ui::library::refresh_history(app);
    app.refresh_saved();
    refresh_devices(app);
}

fn failed(app: &mut App, e: impl ToString) {
    app.sync.busy = None;
    app.sync.error = Some(e.to_string());
}

fn can_sign_in(s: &SyncUi) -> bool {
    s.server.text.trim().len() > "https://".len()
        && !s.username.text.trim().is_empty()
        && !s.password.text.is_empty()
        && s.busy.is_none()
}

fn sign_in(app: &mut App) {
    if !can_sign_in(&app.sync) {
        return;
    }
    let password = std::mem::take(&mut app.sync.password.text);
    let code = std::mem::take(&mut app.sync.code.text);
    app.sync.password.clear();
    app.sync.code.clear();
    let s = &app.sync;
    let req = json!({
        "server": s.server.text.trim(),
        "username": s.username.text.trim(),
        "password": password,
        "code": code.trim(),
        "device_name": device_name(),
    });
    app.sync.busy = Some("Signing in…");
    app.sync.error = None;
    app.be.call(op("sync_sign_in", req), |app, r, _| match r {
        Ok(v) => {
            landed(app, v);
            app.toast(ToastKind::Success, "Signed in to your Vidcue server");
        }
        Err(e) => failed(app, e),
    });
}

fn inspect_link(app: &mut App) {
    let uri = app.sync.link.text.trim().to_owned();
    if uri.is_empty() {
        return;
    }
    app.sync.error = None;
    app.be.call(
        op("sync_pair_inspect", json!({ "uri": uri })),
        move |app, r, _| match r {
            Ok(v) => {
                app.sync.link_check = Some((
                    uri,
                    v["server"].as_str().unwrap_or_default().to_owned(),
                    v["secure"].as_bool().unwrap_or(false),
                ))
            }
            Err(e) => failed(app, e),
        },
    );
}

fn claim_link(app: &mut App) {
    if app.sync.busy.is_some() {
        return;
    }
    let Some((uri, _, _)) = app.sync.link_check.take() else {
        return;
    };
    app.sync.busy = Some("Joining…");
    app.sync.link.clear();
    app.be.call(
        op(
            "sync_pair_claim",
            json!({ "uri": uri, "device_name": device_name() }),
        ),
        |app, r, _| match r {
            // The account is joined but idle until this device confirms it (`confirm_card`).
            Ok(v) => {
                app.sync.busy = None;
                app.sync.error = None;
                app.sync.status = v;
            }
            Err(e) => failed(app, e),
        },
    );
}

/// Confirms a joined account, optionally taking the YouTube sign-in from it too; then sync
/// starts. Android's pairing sheet does the same.
fn confirm_join(app: &mut App, secrets: bool) {
    app.sync.busy = Some("Joining…");
    app.be.call(
        op("sync_pair_confirm", json!({ "secrets": secrets })),
        |app, r, _| match r {
            Ok(v) => {
                landed(app, v);
                app.toast(
                    ToastKind::Success,
                    "This computer now syncs with your account",
                );
            }
            Err(e) => failed(app, e),
        },
    );
}

fn sync_now(app: &mut App) {
    if app.sync.busy.is_some() {
        return;
    }
    app.sync.busy = Some("Syncing…");
    app.be.call(op("sync_now", json!({})), |app, r, _| match r {
        Ok(v) => landed(app, v),
        Err(e) => failed(app, e),
    });
}

fn sign_out(app: &mut App, forget_data: bool) {
    app.sync.confirm_sign_out = false;
    app.be.call(
        op("sync_sign_out", json!({ "forget_data": forget_data })),
        |app, r, _| match r {
            Ok(v) => {
                app.sync.status = v;
                app.sync.devices.clear();
                app.sync.pair = None;
                crate::ui::library_local::refresh(app);
                app.toast(ToastKind::Success, "Signed out of the Vidcue server");
            }
            Err(e) => failed(app, e),
        },
    );
}

fn revoke(app: &mut App, id: String) {
    app.be.call(
        op("sync_revoke", json!({ "device_id": id })),
        |app, r, _| {
            if let Err(e) = r {
                app.toast(ToastKind::Error, e.to_string());
            }
            refresh(app);
        },
    );
}

fn start_pairing(app: &mut App) {
    if app.sync.busy.is_some() || app.sync.pair_password.text.is_empty() {
        return;
    }
    let password = std::mem::take(&mut app.sync.pair_password.text);
    app.sync.pair_password.clear();
    app.sync.busy = Some("Creating a pairing code…");
    app.be.call(
        op("sync_pair_start", json!({ "password": password })),
        |app, r, _| match r {
            Ok(v) => {
                app.sync.busy = None;
                let rows = v["qr"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|r| r.as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                app.sync.pair = Some((rows, v["uri"].as_str().unwrap_or_default().to_owned()));
            }
            Err(e) => failed(app, e),
        },
    );
}

fn set_collection(app: &mut App, name: &'static str, on: bool) {
    app.be.call(
        op("sync_settings", json!({ "collections": { name: on } })),
        |app, r, _| match r {
            Ok(v) => app.sync.status = v,
            Err(e) => app.toast(ToastKind::Error, e.to_string()),
        },
    );
}

/// Engine events from the sync loop (non-Android builds queue them): refresh what changed.
pub fn drain_events(app: &mut App) -> bool {
    let events = app.be.engine.take_sync_events();
    if events.is_empty() {
        return false;
    }
    let (mut library, mut filters, mut playlists) = (false, false, false);
    for (name, detail) in events {
        match (name.as_str(), detail.as_str()) {
            // The status event carries the status itself; no server round trip.
            ("sync_status", text) => {
                if let Ok(v) = serde_json::from_str::<Value>(text) {
                    app.sync.status = v;
                }
            }
            ("history_updated", _) => crate::ui::library::refresh_history(app),
            ("offline_playlists_changed", _) => crate::ui::library::refresh_playlists(app),
            ("library_changed", "saved") => app.refresh_saved(),
            ("library_changed", "filters") => filters = true,
            ("library_changed", "playlists") | ("shared_playlist_changed", _) => playlists = true,
            ("library_changed", _) => library = true,
            ("config_changed", _) => reload_config(app),
            // A YouTube session synced from another device was installed.
            ("account_updated", _) => crate::ui::account::refresh(app),
            _ => {}
        }
    }
    if library || filters || playlists {
        crate::ui::library_local::refresh(app);
    }
    if playlists && let crate::app::Route::LocalPlaylist(id) = app.route.clone() {
        crate::ui::library_local::refresh_entries(app, &id);
    }
    if filters {
        // Feeds already loaded still show what is now hidden.
        app.feeds.pages.clear();
        if let crate::app::Route::Feed(f) = app.route.clone() {
            crate::feed::ensure(app, &f, true);
        }
    }
    true
}

/// Clears the typed password and codes, e.g. when the Sync section is left.
pub fn forget_secrets(app: &mut App) {
    let s = &mut app.sync;
    for field in [&mut s.password, &mut s.code, &mut s.pair_password] {
        if !field.text.is_empty() {
            field.clear();
        }
    }
}

/// Settings changed on another device: read them again and apply them here.
fn reload_config(app: &mut App) {
    app.be.call(op("settings", json!({})), |app, r, _| {
        if let Ok(config) = r.and_then(|v| {
            serde_json::from_value::<vidcue_core::Config>(v).map_err(|e| crate::backend::ApiError {
                code: "provider_error",
                message: e.to_string(),
                retry_after_ms: None,
            })
        }) {
            app.apply_config(config);
        }
    });
}

// ---------------------------------------------------------------------------------------------
// View

/// "3 min ago" for a millisecond timestamp.
pub fn ago(ms: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let s = ((now - ms) / 1000).max(0);
    match s {
        0..=59 => "Just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86_400),
    }
}

fn state_line(status: &Value) -> (String, bool) {
    let state = status["state"].as_str().unwrap_or("idle");
    let notice = status["notice"].as_str().filter(|n| !n.is_empty());
    let last = status["last_success"].as_i64().unwrap_or(0);
    let pending = status["pending"].as_u64().unwrap_or(0);
    let text = match state {
        "syncing" => "Syncing…".to_owned(),
        "offline" => "Offline: changes wait until the server is reachable".to_owned(),
        "error" => notice.unwrap_or("Sync failed").to_owned(),
        _ if status["awaiting_confirmation"].as_bool() == Some(true) => {
            "Waiting for the other device to confirm".to_owned()
        }
        _ if last > 0 => format!(
            "Up to date · {}{}",
            ago(last),
            if pending > 0 {
                format!(" · {pending} waiting")
            } else {
                String::new()
            }
        ),
        _ => "Not synced yet".to_owned(),
    };
    (text, state == "error")
}

/// The Settings → Sync section.
pub fn settings(app: &App) -> El {
    let t = &app.theme;
    let s = &app.sync;
    let mut c = col().w_full().gap(18.0);
    if let Some(e) = &s.error {
        c = c.child(
            row()
                .w_full()
                .px(16.0)
                .py(12.0)
                .gap(12.0)
                .center_y()
                .radius(sz::R_MD)
                .fill(t.error.alpha(0.14))
                .child(icon(&ic::WARNING, 16.0).color(t.error))
                .child(
                    text(e)
                        .font(ty::BODY)
                        .color(t.text)
                        .grow(1.0)
                        .shrink()
                        .wrap(),
                ),
        );
    }
    if s.signed_in() && s.status["awaiting_confirmation"].as_bool() == Some(true) {
        c = c.child(confirm_card(app));
    } else if s.signed_in() {
        c = c
            .child(account_card(app))
            .child(devices_card(app))
            .child(recs_card(app))
            .child(collections_card(app));
    } else {
        c = c.child(sign_in_card(app)).child(link_card(app));
    }
    c
}

fn card(t: &crate::theme::Theme, title: &str) -> El {
    widgets::panel(t)
        .w_full()
        .px(20.0)
        .py(18.0)
        .gap(12.0)
        .child(text(title).font(ty::H3).color(t.text))
}

/// After joining from a link: which account this is, and whether to take its YouTube sign-in.
fn confirm_card(app: &App) -> El {
    let t = &app.theme;
    let st = &app.sync.status;
    let user = st["username"].as_str().unwrap_or_default();
    let server = st["server"].as_str().unwrap_or_default();
    card(t, "Join this account?")
        .child(
            text(format!("{user} on {server}"))
                .font(ty::BODY_STRONG)
                .color(t.text)
                .wrap(),
        )
        .child(
            text("Playlists, history, saved items and settings will sync with this computer.")
                .font(ty::BODY)
                .color(t.muted)
                .wrap(),
        )
        .child(
            row()
                .gap(10.0)
                .pt(4.0)
                .flex_wrap()
                .child(
                    button(t, "Join", Some(&ic::CHECK), Kind::Primary)
                        .key("sync-confirm")
                        .on_press(|a: &mut App| confirm_join(a, false)),
                )
                .child(
                    button(t, "Join with YouTube sign-in", None, Kind::Secondary)
                        .key("sync-confirm-secrets")
                        .on_press(|a: &mut App| confirm_join(a, true)),
                )
                .child(
                    button(t, "Cancel", None, Kind::Ghost)
                        .on_press(|a: &mut App| sign_out(a, false)),
                ),
        )
}

fn sign_in_card(app: &App) -> El {
    let t = &app.theme;
    let s = &app.sync;
    let f = |edit: &TextEdit, hint: &str, key: &str, access: fn(&mut App) -> &mut TextEdit| {
        widgets::field(t, edit, hint, access)
            .key(key.to_owned())
            .label(hint)
            .w_full()
    };
    let ready = can_sign_in(s);
    let mut submit = button(
        t,
        s.busy.unwrap_or("Sign in"),
        Some(&ic::USER),
        Kind::Primary,
    )
    .key("sync-sign-in");
    if ready {
        submit = submit.on_press(sign_in);
    } else {
        submit = submit.dim(0.5);
    }
    card(t, "Vidcue account")
        .child(
            text("Sync playlists, history, saved items and settings with your phone through your own Vidcue server.")
                .font(ty::BODY)
                .color(t.muted)
                .wrap(),
        )
        .child(f(&s.server, "Server address", "sync-server", |a| &mut a.sync.server))
        .child(f(&s.username, "Username", "sync-username", |a| &mut a.sync.username))
        .child(
            f(&s.password, "Password", "sync-password", |a| &mut a.sync.password).on_submit(
                |a: &mut App, tx: &mut Tx| {
                    sign_in(a);
                    tx.blur();
                },
            ),
        )
        .child(f(
            &s.code,
            "Authenticator code (if you set one up)",
            "sync-code",
            |a| &mut a.sync.code,
        ))
        .child(row().pt(4.0).child(submit))
}

fn link_card(app: &App) -> El {
    let t = &app.theme;
    let s = &app.sync;
    let mut c = card(t, "Join from another device")
        .child(
            text(
                "On your phone, open Vidcue account → Add a device, then paste the link it shows.",
            )
            .font(ty::BODY)
            .color(t.muted)
            .wrap(),
        )
        .child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .child(
                    widgets::field(t, &s.link, "vidcue://pair…", |a| &mut a.sync.link)
                        .key("sync-link")
                        .label("Pairing link")
                        .grow(1.0)
                        .min_w(0.0)
                        .on_submit(|a: &mut App, tx: &mut Tx| {
                            inspect_link(a);
                            tx.blur();
                        }),
                )
                .child(button(t, "Check", None, Kind::Secondary).on_press(inspect_link)),
        );
    if let Some((_, origin, secure)) = &s.link_check {
        c =
            c.child(
                row()
                    .w_full()
                    .gap(10.0)
                    .center_y()
                    .flex_wrap()
                    .child(
                        icon(if *secure { &ic::KEY } else { &ic::WARNING }, 16.0)
                            .color(if *secure { t.success } else { t.warning }),
                    )
                    .child(
                        text(if *secure {
                            format!("Join {origin}?")
                        } else {
                            format!("{origin} is not encrypted (http). Join anyway?")
                        })
                        .font(ty::BODY_STRONG)
                        .color(t.text)
                        .shrink(),
                    )
                    .child(button(t, "Join", Some(&ic::CHECK), Kind::Primary).on_press(claim_link))
                    .child(
                        button(t, "Cancel", None, Kind::Ghost)
                            .on_press(|a: &mut App| a.sync.link_check = None),
                    ),
            );
    }
    c
}

fn account_card(app: &App) -> El {
    let t = &app.theme;
    let s = &app.sync;
    let st = &s.status;
    let user = st["username"].as_str().unwrap_or_default();
    let server = st["server"].as_str().unwrap_or_default();
    let host = url::Url::parse(server)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| server.to_owned());
    let (line, bad) = state_line(st);
    let mut actions = row().gap(10.0).pt(4.0).flex_wrap();
    actions = actions.child(
        button(
            t,
            s.busy.unwrap_or("Sync now"),
            Some(&ic::REFRESH),
            Kind::Secondary,
        )
        .key("sync-now")
        .on_press(sync_now),
    );
    if s.confirm_sign_out {
        actions = actions
            .child(
                button(t, "Sign out", Some(&ic::LOGOUT), Kind::Danger)
                    .on_press(|a: &mut App| sign_out(a, false)),
            )
            .child(
                button(t, "Sign out and remove synced data", None, Kind::Ghost)
                    .on_press(|a: &mut App| sign_out(a, true)),
            )
            .child(
                button(t, "Cancel", None, Kind::Ghost)
                    .on_press(|a: &mut App| a.sync.confirm_sign_out = false),
            );
    } else {
        actions = actions.child(
            button(t, "Sign out", Some(&ic::LOGOUT), Kind::Ghost)
                .key("sync-sign-out")
                .on_press(|a: &mut App| a.sync.confirm_sign_out = true),
        );
    }
    card(t, "Vidcue account")
        .child(
            row()
                .gap(14.0)
                .center_y()
                .child(
                    el().size(44.0, 44.0)
                        .radius(22.0)
                        .center()
                        .fill(t.accent_soft)
                        .child(
                            text(
                                user.chars()
                                    .next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_default(),
                            )
                            .font(ty::H3)
                            .color(t.accent),
                        ),
                )
                .child(
                    col()
                        .gap(2.0)
                        .child(text(user).font(ty::BODY_STRONG).color(t.text))
                        .child(text(host).font(ty::SMALL).color(t.muted)),
                ),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(el().size(8.0, 8.0).radius(4.0).fill(if bad {
                    t.error
                } else if st["connected"].as_bool() == Some(true) {
                    t.success
                } else {
                    t.faint
                }))
                .child(
                    text(line)
                        .font(ty::SMALL)
                        .color(if bad { t.error } else { t.muted }),
                ),
        )
        .child(actions)
}

fn devices_card(app: &App) -> El {
    let t = &app.theme;
    let s = &app.sync;
    let mut c = card(t, "Devices");
    for d in &s.devices {
        let id = d["id"].as_str().unwrap_or_default().to_owned();
        let current = d["current"].as_bool().unwrap_or(false);
        let online = d["online"].as_bool().unwrap_or(false);
        let name = d["name"].as_str().unwrap_or("Device");
        let mut detail = if current {
            "This computer".to_owned()
        } else if online {
            "Online".to_owned()
        } else {
            d["last_seen"]
                .as_i64()
                .filter(|ms| *ms > 0)
                .map(|ms| format!("Seen {}", ago(ms).to_lowercase()))
                .unwrap_or_default()
        };
        if let Some(title) = d["now_playing"]["title"].as_str() {
            detail.push_str(&format!(" · Playing “{title}”"));
        }
        let mut r = row()
            .key(format!("device-{id}"))
            .w_full()
            .h(52.0)
            .gap(12.0)
            .center_y()
            .child(el().size(8.0, 8.0).radius(4.0).fill(if online || current {
                t.success
            } else {
                t.faint
            }))
            .child(
                col()
                    .grow(1.0)
                    .shrink()
                    .gap(2.0)
                    .child(text(name).font(ty::BODY_STRONG).color(t.text).truncate())
                    .child(text(detail).font(ty::SMALL).color(t.muted).truncate()),
            );
        if !current {
            r = r.child(
                widgets::icon_button(t, &ic::TRASH, "Remove this device", 34.0)
                    .on_press(move |a: &mut App| revoke(a, id.clone())),
            );
        }
        c = c.child(r);
    }
    // Pair a phone: the server makes a one-time link, shown as a QR code to scan.
    match &s.pair {
        Some((rows, uri)) => {
            let uri = uri.clone();
            c = c.child(
                row()
                    .w_full()
                    .pt(8.0)
                    .gap(20.0)
                    .items_start()
                    .child(qr(rows, 4.0))
                    .child(
                        col()
                            .gap(10.0)
                            .grow(1.0)
                            .shrink()
                            .child(
                                text("Scan with the phone's camera, or open Vidcue account → Join on the phone.")
                                    .font(ty::BODY)
                                    .color(t.text)
                                    .wrap(),
                            )
                            .child(
                                row()
                                    .gap(8.0)
                                    .child(
                                        button(t, "Copy link", Some(&ic::COPY), Kind::Secondary)
                                            .on_press_tx(move |a: &mut App, tx: &mut Tx| {
                                                tx.copy(uri.clone());
                                                a.toast(ToastKind::Success, "Link copied");
                                            }),
                                    )
                                    .child(
                                        button(t, "Done", None, Kind::Ghost)
                                            .on_press(|a: &mut App| {
                                                a.sync.pair = None;
                                                refresh(a);
                                            }),
                                    ),
                            ),
                    ),
            );
        }
        None => {
            c = c.child(
                row()
                    .w_full()
                    .pt(6.0)
                    .gap(10.0)
                    .center_y()
                    .child(
                        widgets::field(t, &s.pair_password, "Password, to add a device", |a| {
                            &mut a.sync.pair_password
                        })
                        .key("sync-pair-password")
                        .label("Password to add a device")
                        .grow(1.0)
                        .min_w(0.0)
                        .on_submit(|a: &mut App, tx: &mut Tx| {
                            start_pairing(a);
                            tx.blur();
                        }),
                    )
                    .child(
                        button(t, "Add a device", Some(&ic::PLUS), Kind::Secondary)
                            .key("sync-pair")
                            .on_press(start_pairing),
                    ),
            );
        }
    }
    c
}

/// The server's recommendations: off until switched on, as on Android.
fn recs_card(app: &App) -> El {
    let t = &app.theme;
    let st = &app.sync.recs_settings;
    let on = st["enabled"].as_bool().unwrap_or(false);
    let mut c = card(t, "Recommendations").child(widgets::setting_row(
        t,
        "Recommend from what I play",
        "Shown on Home as “For you”",
        widgets::switch(t, "recs-enabled", on)
            .on_press(move |a: &mut App| set_recs(a, "enabled", !on)),
    ));
    if on {
        let music = st["music"].as_bool().unwrap_or(true);
        c = c.child(widgets::setting_row(
            t,
            "Similar artists",
            "",
            widgets::switch(t, "recs-music", music)
                .on_press(move |a: &mut App| set_recs(a, "music", !music)),
        ));
    }
    c
}

fn collections_card(app: &App) -> El {
    let t = &app.theme;
    let map = &app.sync.status["collections"];
    let mut c = card(t, "What syncs");
    for (name, label) in COLLECTIONS {
        let on = map[name].as_bool().unwrap_or(true);
        c = c.child(widgets::setting_row(
            t,
            label,
            "",
            widgets::switch(t, &format!("sync-{name}"), on)
                .on_press(move |a: &mut App| set_collection(a, name, !on)),
        ));
    }
    c
}

/// A QR code from rows of "0"/"1", dark runs merged into one bar each. Always dark on white,
/// whatever the theme, so phone cameras read it.
pub fn qr(rows: &[String], module: f32) -> El {
    let n = rows.len() as f32;
    let mut c = el()
        .key("pair-qr")
        .size(n * module, n * module)
        .radius(8.0)
        .fill(Color::WHITE)
        .label("Pairing QR code");
    for (y, line) in rows.iter().enumerate() {
        for (start, len) in dark_runs(line) {
            c = c.child(
                el().absolute(start as f32 * module, y as f32 * module)
                    .size(len as f32 * module, module)
                    .fill(Color::BLACK),
            );
        }
    }
    c
}

/// (start, length) of each run of "1"s in a QR row.
fn dark_runs(line: &str) -> Vec<(usize, usize)> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut x = 0;
    while x < bytes.len() {
        if bytes[x] == b'1' {
            let start = x;
            while x < bytes.len() && bytes[x] == b'1' {
                x += 1;
            }
            out.push((start, x - start));
        } else {
            x += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_lines_name_the_state() {
        assert_eq!(state_line(&json!({"state": "syncing"})).0, "Syncing…");
        let (line, bad) = state_line(&json!({"state": "error", "notice": "Server unreachable"}));
        assert!(bad && line == "Server unreachable");
        assert_eq!(state_line(&json!({"state": "idle"})).0, "Not synced yet");
        assert!(device_name().starts_with("Vidcue"));
    }

    #[test]
    fn qr_merges_dark_runs() {
        assert_eq!(dark_runs("0110"), [(1, 2)]);
        assert_eq!(dark_runs("1111"), [(0, 4)]);
        assert_eq!(dark_runs("0000"), []);
        assert_eq!(dark_runs("1001"), [(0, 1), (3, 1)]);
    }

    #[test]
    fn every_engine_collection_has_a_name() {
        let names: Vec<&str> = COLLECTIONS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vidcue_sync_collections());
    }

    fn vidcue_sync_collections() -> Vec<&'static str> {
        vec![
            "positions",
            "history",
            "saved",
            "playlists",
            "offline_playlists",
            "subscriptions",
            "settings",
            "eq_presets",
            "filters",
            "secrets",
        ]
    }
}
