//! App state, navigation and the `setup`/`poll` hooks Rune calls.

use std::collections::HashMap;
use std::time::Duration;

use rune::{Tx, Waker};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vidcue_core::{BrowseRequest, Config, Media};

use crate::backend::{Backend, op};
use crate::downloads::Downloads;
use crate::feed::Feeds;
use crate::images::Images;
use crate::movies::Movies;
use crate::player::session::Player;
use crate::theme::Theme;
use crate::ui::account::AccountUi;
use crate::ui::library::LibraryUi;
use crate::ui::settings::SettingsUi;

/// The three engine catalogs, numbered as the engine numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Home = 1,
    Music = 2,
    Radio = 3,
}

impl Tab {
    pub fn id(self) -> u8 {
        self as u8
    }
    pub fn music(self) -> bool {
        self == Tab::Music
    }
    pub fn label(self) -> &'static str {
        match self {
            Tab::Home => "Home",
            Tab::Music => "Music",
            Tab::Radio => "Radio",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScope {
    Catalog(Tab),
    Movies,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FeedRoute {
    pub tab: Tab,
    pub request: BrowseRequest,
    pub title: String,
}

impl FeedRoute {
    pub fn root(tab: Tab) -> Self {
        let request = match tab {
            Tab::Home | Tab::Music => BrowseRequest::Home,
            Tab::Radio => BrowseRequest::Cliamp,
        };
        Self {
            tab,
            request,
            title: tab.label().to_owned(),
        }
    }
    pub fn key(&self) -> String {
        format!(
            "{}:{}",
            self.tab.id(),
            serde_json::to_string(&self.request).unwrap_or_default()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryTab {
    Saved,
    History,
    Playlists,
    Queue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    Appearance,
    Playback,
    Audio,
    Downloads,
    Network,
    Privacy,
    Advanced,
    Alpha,
    About,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    Feed(FeedRoute),
    Watch,
    Downloads,
    Library(LibraryTab),
    OfflinePlaylist(String),
    Movies,
    Settings(SettingsSection),
    Account,
}

impl Route {
    /// The sidebar entry this route lives under.
    pub fn section(&self) -> &'static str {
        match self {
            Route::Feed(f) => match f.tab {
                Tab::Home => "home",
                Tab::Music => "music",
                Tab::Radio => "radio",
            },
            Route::Watch => "watch",
            Route::Downloads => "downloads",
            Route::Library(_) | Route::OfflinePlaylist(_) => "library",
            Route::Movies => "movies",
            Route::Settings(_) => "settings",
            Route::Account => "account",
        }
    }
}

/// Desktop-only preferences: what Android keeps in SharedPreferences and the desktop shell's
/// own choices. Engine settings stay in the engine's `Config`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub onboarding_version: u32,
    pub sidebar_compact: bool,
    pub ambient: bool,
    pub hover_previews: bool,
    pub effects: bool,
    pub volume: f64,
    pub speed: f64,
    pub captions: bool,
    pub alpha_movies: bool,
    pub alpha_notice_accepted: bool,
    pub tmdb_key: String,
    pub recent_media: Vec<String>,
    pub recent_radio: Vec<String>,
    pub show_lyrics: bool,
    pub grid_dense: bool,
    /// Desktop cards show views and upload dates; applied once so the user can turn them off.
    pub card_details_applied: bool,
    /// A release tag the user chose to skip: the startup offer stays quiet for it.
    pub skipped_update: Option<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            onboarding_version: 0,
            sidebar_compact: false,
            ambient: true,
            hover_previews: true,
            effects: true,
            volume: 100.0,
            speed: 1.0,
            captions: false,
            alpha_movies: false,
            alpha_notice_accepted: false,
            tmdb_key: String::new(),
            recent_media: Vec::new(),
            recent_radio: Vec::new(),
            show_lyrics: false,
            grid_dense: false,
            card_details_applied: false,
            skipped_update: None,
        }
    }
}

pub const ONBOARDING_VERSION: u32 = 1;
const RECENT_LIMIT: usize = 12;

impl Prefs {
    pub fn load(be: &Backend) -> Self {
        std::fs::read(be.paths.file("desktop.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, be: &Backend) {
        if be.offline {
            return;
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = vidcue_providers::paths::atomic_write(&be.paths.file("desktop.json"), &bytes);
        }
    }
    /// Remembers a search, newest first, without duplicates.
    pub fn remember(&mut self, radio: bool, query: &str) {
        let q = query.trim();
        if q.is_empty() {
            return;
        }
        let list = if radio {
            &mut self.recent_radio
        } else {
            &mut self.recent_media
        };
        list.retain(|x| !x.eq_ignore_ascii_case(q));
        list.insert(0, q.to_owned());
        list.truncate(RECENT_LIMIT);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub text: String,
    pub action: Option<(String, ToastAction)>,
    pub until: f64,
}

#[derive(Clone, Debug)]
pub enum ToastAction {
    UndoSponsorSkip(f64),
    OpenAccount,
    OpenDownloads,
    Retry,
}

/// A small in-app menu anchored to an element (Linux has no native pop-up menus).
#[derive(Clone, Debug)]
pub struct MenuState {
    pub anchor: String,
    pub items: Vec<MenuItem>,
}

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<&'static rune::Icon>,
    pub action: MenuAction,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub enum MenuAction {
    Play(Box<Media>),
    PlayAudio(Box<Media>),
    Enqueue(Box<Media>),
    Save(Box<Media>),
    Download(Box<Media>, bool),
    OpenChannel(Box<Media>),
    CopyLink(String),
    OpenInBrowser(String),
    AddToPlaylist(Box<Media>, String),
    NewPlaylistWith(Box<Media>),
    Preview(Box<Media>),
}

pub struct App {
    pub be: Backend,
    pub images: Images,
    pub theme: Theme,
    pub config: Config,
    pub config_loaded: bool,
    /// The system's dark-mode setting as the view last saw it (`themes::observe_system_dark`).
    pub system_dark: std::cell::Cell<Option<bool>>,
    pub prefs: Prefs,
    pub route: Route,
    pub back: Vec<Route>,
    pub forward: Vec<Route>,
    pub feeds: Feeds,
    pub search: rune::TextEdit,
    pub search_open: bool,
    pub player: Player,
    pub downloads: Downloads,
    pub movies: Movies,
    pub library: LibraryUi,
    pub settings: SettingsUi,
    pub account: AccountUi,
    pub toasts: Vec<Toast>,
    pub toast_seq: u64,
    pub menu: Option<MenuState>,
    pub onboarding: Option<usize>,
    pub update: crate::updater::Updater,
    pub clock: f64,
    pub waker: Option<Waker>,
    pub saved_urls: std::collections::HashSet<String>,
    pub started: bool,
    pub scratch: HashMap<String, String>,
    pub preview: crate::player::preview::Preview,
    /// Sparkle bursts by element key, with their start on the app clock.
    pub bursts: HashMap<String, f64>,
}

impl App {
    pub fn new() -> Self {
        let headless = rune::tooling::headless_run().is_some()
            || std::env::var_os("VIDCUE_DESKTOP_OFFLINE").is_some();
        Self::build(headless)
    }

    /// An app with a throwaway, network-free engine: headless tests.
    #[cfg(test)]
    pub fn offline_for_tests() -> Self {
        Self::build(true)
    }

    fn build(headless: bool) -> Self {
        let be = Backend::start(headless).expect("The Vidcue engine could not start");
        let prefs = Prefs::load(&be);
        let images = Images::new(&be);
        let onboarding = (prefs.onboarding_version < ONBOARDING_VERSION && !headless).then_some(0);
        Self {
            images,
            theme: Theme::fallback(),
            config: Config::default(),
            config_loaded: false,
            system_dark: std::cell::Cell::new(None),
            route: Route::Feed(FeedRoute::root(Tab::Home)),
            back: Vec::new(),
            forward: Vec::new(),
            feeds: Feeds::default(),
            search: rune::TextEdit::new(""),
            search_open: false,
            player: Player::new(),
            downloads: Downloads::new(&be),
            movies: Movies::new(),
            library: LibraryUi::default(),
            settings: SettingsUi::default(),
            account: AccountUi::default(),
            toasts: Vec::new(),
            toast_seq: 0,
            menu: None,
            onboarding,
            update: crate::updater::Updater::new(headless),
            clock: 0.0,
            waker: None,
            saved_urls: Default::default(),
            started: false,
            scratch: HashMap::new(),
            preview: Default::default(),
            bursts: HashMap::new(),
            prefs,
            be,
        }
    }

    pub fn offline(&self) -> bool {
        self.be.offline
    }

    // ---- navigation ----

    pub fn go(&mut self, route: Route) {
        if self.route == route {
            return;
        }
        let old = std::mem::replace(&mut self.route, route);
        self.back.push(old);
        if self.back.len() > 64 {
            self.back.remove(0);
        }
        self.forward.clear();
        self.menu = None;
        self.on_route();
    }

    /// Goes to a sidebar destination: history restarts there.
    pub fn go_root(&mut self, route: Route) {
        if self.route != route {
            let old = std::mem::replace(&mut self.route, route);
            self.back.push(old);
        }
        self.forward.clear();
        self.menu = None;
        self.on_route();
    }

    pub fn go_back(&mut self) {
        if let Some(r) = self.back.pop() {
            let old = std::mem::replace(&mut self.route, r);
            self.forward.push(old);
            self.menu = None;
            self.on_route();
        } else if self.route == Route::Watch {
            // Leaving the player always lands somewhere, even with no history behind it.
            let old = std::mem::replace(&mut self.route, Route::Feed(FeedRoute::root(Tab::Home)));
            self.forward.push(old);
            self.menu = None;
            self.on_route();
        }
    }

    pub fn go_forward(&mut self) {
        if let Some(r) = self.forward.pop() {
            let old = std::mem::replace(&mut self.route, r);
            self.back.push(old);
            self.menu = None;
            self.on_route();
        }
    }

    /// Starts whatever the new page needs.
    fn on_route(&mut self) {
        match self.route.clone() {
            Route::Feed(f) => crate::feed::ensure(self, &f, false),
            Route::Library(tab) => crate::ui::library::enter(self, tab),
            Route::OfflinePlaylist(_) => crate::ui::library::enter(self, LibraryTab::Playlists),
            Route::Downloads => crate::downloads::enter(self),
            Route::Movies => crate::movies::enter(self),
            Route::Account => crate::ui::account::enter(self),
            Route::Settings(_) => crate::ui::settings::enter(self),
            Route::Watch => {}
        }
    }

    pub fn open_feed(&mut self, tab: Tab, request: BrowseRequest, title: impl Into<String>) {
        self.go(Route::Feed(FeedRoute {
            tab,
            request,
            title: title.into(),
        }));
    }

    /// Opens a playlist, album, artist or channel card.
    pub fn open_collection(&mut self, tab: Tab, media: &Media) {
        let id = media
            .browse_id
            .clone()
            .or_else(|| Some(media.channel_id.clone()).filter(|c| !c.is_empty()))
            .unwrap_or_else(|| media.url.clone());
        self.open_feed(tab, BrowseRequest::Collection(id), media.title.clone());
    }

    pub fn open_channel(&mut self, tab: Tab, media: &Media) {
        if !media.channel_id.is_empty() {
            self.open_feed(
                tab,
                BrowseRequest::Collection(media.channel_id.clone()),
                media.creator.clone(),
            );
        }
    }

    pub fn search_submit(&mut self, query: &str) {
        let q = query.trim().to_owned();
        if q.is_empty() {
            return;
        }
        let scope = self.search_scope();
        if scope == SearchScope::Movies {
            self.movies.search.replace_text(&q);
            crate::movies::search(self);
            self.search_open = false;
            return;
        }
        let radio = scope == SearchScope::Catalog(Tab::Radio);
        if !radio && let Some(media) = crate::links::classify(&q) {
            self.prefs.remember(false, &q);
            self.prefs.save(&self.be);
            match media {
                crate::links::Linked::Video(m) => {
                    crate::player::session::play(self, *m, Default::default())
                }
                crate::links::Linked::Playlist(id, title) => {
                    self.open_feed(Tab::Home, BrowseRequest::Collection(id), title)
                }
            }
            return;
        }
        let SearchScope::Catalog(tab) = scope else {
            return;
        };
        self.prefs.remember(radio, &q);
        self.prefs.save(&self.be);
        let request = if tab == Tab::Radio {
            BrowseRequest::RadioSearch(q.clone())
        } else {
            BrowseRequest::Search(q.clone())
        };
        self.open_feed(tab, request, format!("“{q}”"));
        self.search_open = false;
    }

    /// What the top bar searches: the catalog of the page you are on. The player searches the
    /// kind that is playing; library and settings pages search the last catalog visited.
    pub fn search_scope(&self) -> SearchScope {
        let from_route = |r: &Route| match r {
            Route::Feed(f) => Some(SearchScope::Catalog(f.tab)),
            Route::Movies => Some(SearchScope::Movies),
            Route::Watch => self.player.now.as_ref().map(|n| match n.media.kind {
                vidcue_core::MediaKind::Song
                | vidcue_core::MediaKind::Album
                | vidcue_core::MediaKind::Artist => SearchScope::Catalog(Tab::Music),
                vidcue_core::MediaKind::Station => SearchScope::Catalog(Tab::Radio),
                _ if matches!(n.source, crate::player::session::Source::Movie(_)) => {
                    SearchScope::Movies
                }
                _ => SearchScope::Catalog(Tab::Home),
            }),
            _ => None,
        };
        from_route(&self.route)
            .or_else(|| self.back.iter().rev().find_map(from_route))
            .unwrap_or(SearchScope::Catalog(Tab::Home))
    }

    // ---- toasts ----

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toast_with(kind, text, None);
    }

    pub fn toast_with(
        &mut self,
        kind: ToastKind,
        text: impl Into<String>,
        action: Option<(String, ToastAction)>,
    ) {
        self.toast_seq += 1;
        let text = text.into();
        let life = if kind == ToastKind::Error { 7.0 } else { 4.0 }
            + if action.is_some() { 2.0 } else { 0.0 };
        self.toasts.retain(|t| t.text != text);
        self.toasts.push(Toast {
            id: self.toast_seq,
            kind,
            text,
            action,
            until: self.clock + life,
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn run_toast(&mut self, id: u64, tx: &mut Tx) {
        let Some(t) = self
            .toasts
            .iter()
            .position(|t| t.id == id)
            .map(|i| self.toasts.remove(i))
        else {
            return;
        };
        if let Some((_, action)) = t.action {
            match action {
                ToastAction::UndoSponsorSkip(to) => self.player.seek_to(to, true),
                ToastAction::OpenAccount => self.go_root(Route::Account),
                ToastAction::OpenDownloads => self.go_root(Route::Downloads),
                ToastAction::Retry => crate::player::session::retry(self, tx),
            }
        }
    }

    // ---- settings ----

    pub fn apply_config(&mut self, config: Config) {
        let theme_changed = config.theme != self.config.theme || !self.config_loaded;
        self.config = config;
        self.config_loaded = true;
        if theme_changed {
            crate::themes::reload(self);
        }
        self.player.apply_config(&self.config);
        self.images
            .configure(&self.be, self.config.proxy.as_deref());
    }

    /// Saves engine settings, applying them once the engine accepts them.
    pub fn save_config(&mut self, config: Config) {
        let previous = self.config.clone();
        self.apply_config(config.clone());
        self.be.call(
            op("save_settings", json!({ "config": config })),
            move |app, r, _| {
                if let Err(e) = r {
                    app.apply_config(previous);
                    app.toast(ToastKind::Error, format!("Settings were not saved: {e}"));
                }
            },
        );
    }

    pub fn edit_config(&mut self, f: impl FnOnce(&mut Config)) {
        let mut c = self.config.clone();
        f(&mut c);
        if let Err(e) = c.validate() {
            self.toast(ToastKind::Error, e.to_string());
            return;
        }
        self.save_config(c);
    }

    pub fn edit_prefs(&mut self, f: impl FnOnce(&mut Prefs)) {
        f(&mut self.prefs);
        self.prefs.save(&self.be);
    }

    pub fn refresh_saved(&mut self) {
        self.be.call(op("saved", json!({})), |app, r, _| {
            if let Ok(Value::Array(items)) = r {
                app.saved_urls = items
                    .iter()
                    .filter_map(|m| m["url"].as_str().map(str::to_owned))
                    .collect();
                app.library.saved = items
                    .into_iter()
                    .filter_map(|m| serde_json::from_value(m).ok())
                    .collect();
            }
        });
    }

    pub fn toggle_saved(&mut self, media: &Media) {
        let was = self.saved_urls.contains(&media.url);
        if was {
            self.saved_urls.remove(&media.url);
        } else {
            self.saved_urls.insert(media.url.clone());
            crate::fx::burst(self, &format!("save-{}", media.url));
        }
        let title = media.title.clone();
        self.be.call(
            op("save_media", json!({ "media": media })),
            move |app, r, _| match r {
                Ok(v) => {
                    let saved = v["saved"].as_bool().unwrap_or(!was);
                    app.toast(
                        ToastKind::Success,
                        if saved {
                            format!("Saved “{title}”")
                        } else {
                            format!("Removed “{title}” from saved")
                        },
                    );
                    app.refresh_saved();
                }
                Err(e) => {
                    app.toast(ToastKind::Error, e.to_string());
                    app.refresh_saved();
                }
            },
        );
    }

    pub fn enqueue(&mut self, media: &Media) {
        let title = media.title.clone();
        self.be.call(
            op("enqueue", json!({ "media": media })),
            move |app, r, _| match r {
                Ok(v) => {
                    app.toast(
                        ToastKind::Success,
                        format!(
                            "Queued “{title}” · {} in queue",
                            v["count"].as_u64().unwrap_or(0)
                        ),
                    );
                    app.player.refresh_up_next(&app.be);
                }
                Err(e) => app.toast(ToastKind::Error, e.to_string()),
            },
        );
    }

    pub fn run_menu(&mut self, action: MenuAction, tx: &mut Tx) {
        self.menu = None;
        match action {
            MenuAction::Play(m) => crate::player::session::play(self, *m, Default::default()),
            MenuAction::PlayAudio(m) => crate::player::session::play(
                self,
                *m,
                crate::player::session::PlayOptions {
                    audio: Some(true),
                    ..Default::default()
                },
            ),
            MenuAction::Enqueue(m) => self.enqueue(&m),
            MenuAction::Save(m) => self.toggle_saved(&m),
            MenuAction::Download(m, audio) => crate::downloads::add(self, &m, audio),
            MenuAction::OpenChannel(m) => {
                let tab = if matches!(
                    m.kind,
                    vidcue_core::MediaKind::Song | vidcue_core::MediaKind::Album
                ) {
                    Tab::Music
                } else {
                    Tab::Home
                };
                self.open_channel(tab, &m)
            }
            MenuAction::CopyLink(url) => {
                tx.copy(url);
                self.toast(ToastKind::Success, "Link copied");
            }
            MenuAction::OpenInBrowser(url) => {
                let _ = tx.open_url(&url);
            }
            MenuAction::AddToPlaylist(m, id) => crate::ui::library::add_to_playlist(self, &id, *m),
            MenuAction::NewPlaylistWith(m) => crate::ui::library::new_playlist_with(self, *m),
            MenuAction::Preview(m) => crate::player::preview::start(self, &m),
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// `AppSpec::setup`: the window exists; start background work.
pub fn setup(app: &mut App, waker: Waker) {
    app.waker = Some(waker.clone());
    app.be.set_waker(waker.clone());
    app.images.set_waker(waker.clone());
    app.player.setup(waker.clone(), &app.prefs, app.offline());
    crate::player::preview::setup(app, waker);
}

/// First poll after the first frame: load settings and the first feed.
fn start(app: &mut App) {
    app.started = true;
    if app.offline() {
        app.config_loaded = true;
        return;
    }
    app.be.call(op("settings", json!({})), |app, r, _| {
        match r.and_then(|v| {
            serde_json::from_value::<Config>(v).map_err(|e| crate::backend::ApiError {
                code: "provider_error",
                message: e.to_string(),
                retry_after_ms: None,
            })
        }) {
            Ok(config) => {
                app.apply_config(config);
                if !app.prefs.card_details_applied {
                    app.edit_prefs(|p| p.card_details_applied = true);
                    app.edit_config(|c| {
                        c.card_info.views = true;
                        c.card_info.upload_date = true;
                    });
                }
                crate::downloads::resume(app);
                crate::updater::startup(app);
            }
            Err(e) => app.toast(ToastKind::Error, format!("Settings could not be read: {e}")),
        }
    });
    if let Route::Feed(f) = app.route.clone() {
        crate::feed::ensure(app, &f, false);
    }
    app.refresh_saved();
    app.player.refresh_up_next(&app.be);
    crate::ui::account::refresh(app);
}

/// `AppSpec::poll`: land answers, tick the player, expire toasts.
pub fn poll(app: &mut App, clock: f64, tx: &mut Tx) -> bool {
    app.clock = clock;
    let mut changed = false;
    if !app.started {
        start(app);
        changed = true;
    }
    for landing in app.be.drain() {
        landing(app, tx);
        changed = true;
    }
    changed |= app.images.pump(&app.be);
    changed |= crate::player::session::tick(app, tx);
    changed |= crate::downloads::tick(app);
    changed |= crate::player::preview::tick(app);
    changed |= crate::updater::tick(app, tx);
    app.bursts.retain(|_, t0| clock - *t0 < 2.0);
    let before = app.toasts.len();
    app.toasts.retain(|t| t.until > clock);
    changed |= before != app.toasts.len();
    if let Some(next) = app.toasts.iter().map(|t| t.until).reduce(f64::min) {
        tx.wake_after(Duration::from_secs_f64((next - clock).max(0.05)));
    }
    changed
}
