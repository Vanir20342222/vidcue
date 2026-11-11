//! Account: the optional YouTube/Google sign-in. Four ways to bring a session in (a browser's
//! cookies, a cookies.txt file, a pasted Cookie header, the terminal app's stored session), a
//! reload and a confirmed disconnect. Every cookie import is an explicit button press; nothing
//! here reads a browser, a file or the keyring on its own. Cookie values are never logged or
//! shown, and the paste field is cleared as soon as it is used.

use std::path::PathBuf;

use rune::tok::motion;
use rune::{
    Color, Cx, El, ScrollSpec, Stagger, TextEdit, TextStyle, Transition, Tx, col, el, icon, row,
    text,
};
use serde_json::{Value, json};
use vidcue_services::auth::BrowserProfile;

use crate::app::{App, ToastKind};
use crate::backend::{ApiError, op};
use crate::icons as ic;
use crate::theme::{Theme, sz, ty};
use crate::ui::widgets::{Kind, button, icon_button};

/// The largest cookie document the engine accepts.
pub const MAX_COOKIE_BYTES: u64 = 2 * 1024 * 1024;
/// Highest Google account slot the stepper offers (`/u/N` style index).
pub const MAX_ACCOUNT_INDEX: u32 = 9;

pub struct AccountUi {
    pub connected: bool,
    pub notice: Option<String>,
    /// Browser profile folders found on disk (names only: no cookie database was opened).
    pub profiles: Vec<BrowserProfile>,
    pub profiles_loaded: bool,
    /// The pasted Cookie header. Secure, and cleared after every use.
    pub header: TextEdit,
    /// What is being imported right now, if anything.
    pub busy: Option<String>,
    pub confirm_disconnect: bool,
}

impl Default for AccountUi {
    fn default() -> Self {
        Self {
            connected: false,
            notice: None,
            profiles: Vec::new(),
            profiles_loaded: false,
            header: TextEdit::new("").secure(),
            busy: None,
            confirm_disconnect: false,
        }
    }
}

pub fn enter(app: &mut App) {
    app.account.confirm_disconnect = false;
    refresh(app);
    let paths = app.be.paths.shared.clone();
    app.be.blocking(
        move || vidcue_services::auth::profiles(&paths),
        |app, found, _| {
            app.account.profiles = found;
            app.account.profiles_loaded = true;
        },
    );
}

pub fn refresh(app: &mut App) {
    app.be.call(op("account", json!({})), |app, r, _| {
        if let Ok(v) = r {
            app.account.connected = v["connected"].as_bool().unwrap_or(false);
            app.account.notice = v["notice"].as_str().map(str::to_owned);
        }
    });
}

// ---- actions ----

/// Common tail of every account change: refresh state, refetch feeds, reload saved items, toast.
fn changed(app: &mut App, r: Result<Value, ApiError>, success: &str) {
    app.account.busy = None;
    match r {
        Ok(v) => {
            if let Some(c) = v["connected"].as_bool() {
                app.account.connected = c;
            }
            app.account.notice = v["notice"].as_str().map(str::to_owned);
            app.account.confirm_disconnect = false;
            app.feeds.pages.clear();
            refresh(app);
            app.refresh_saved();
            app.toast(ToastKind::Success, success);
        }
        Err(e) => app.toast(ToastKind::Error, e.message),
    }
}

fn send(app: &mut App, name: &'static str, fields: Value, busy: &str, success: &'static str) {
    if app.account.busy.is_some() {
        return;
    }
    app.account.busy = Some(busy.to_owned());
    app.be
        .call(op(name, fields), move |app, r, _| changed(app, r, success));
}

fn import_browser(app: &mut App, profile: BrowserProfile) {
    if app.account.busy.is_some() {
        return;
    }
    let label = browser_label(&profile.browser);
    app.account.busy = Some(format!("Reading {label} cookies"));
    let (browser, path) = (profile.browser.clone(), profile.path.clone());
    app.be.blocking(
        move || {
            vidcue_services::auth::import_browser(&browser, Some(&path))
                .map_err(|e| vidcue_providers::diagnostic(&format!("{e:#}")))
                .and_then(|s| serde_json::to_value(&s.cookies).map_err(|e| e.to_string()))
        },
        move |app, r, _| {
            app.account.busy = None;
            match r {
                Ok(cookies) => send(
                    app,
                    "import_cookies",
                    json!({ "cookies": cookies }),
                    "Saving the session",
                    "Signed in from your browser",
                ),
                Err(e) => app.toast(
                    ToastKind::Error,
                    format!("Could not read {label} cookies: {e}"),
                ),
            }
        },
    );
}

/// Opens Google's sign-in page in Vidcue's own web view window and imports the YouTube session
/// once the user has signed in there.
fn sign_in_with_google(app: &mut App) {
    if app.account.busy.is_some() {
        return;
    }
    app.account.busy = Some("Sign in in the Google window".into());
    app.be.blocking(crate::signin::sign_in, |app, r, _| {
        app.account.busy = None;
        match r {
            Ok(header) => send(
                app,
                "import_cookies",
                json!({ "header": header }),
                "Saving the session",
                "Signed in with Google",
            ),
            Err(e) => app.toast(ToastKind::Error, e),
        }
    });
}

fn pick_cookie_file(tx: &mut Tx) {
    let options = rune::FileDialogOptions {
        title: "Choose a Netscape cookies.txt".into(),
        ..Default::default()
    }
    .filter("Cookie files", &["txt"])
    .filter("All files", &["*"]);
    tx.open_file(options, |s: &mut App, r, _tx| match r {
        Ok(Some(path)) => import_cookie_file(s, path),
        Ok(None) => {}
        Err(e) => s.toast(ToastKind::Error, format!("File dialog unavailable: {e}")),
    });
}

fn import_cookie_file(app: &mut App, path: PathBuf) {
    if app.account.busy.is_some() {
        return;
    }
    app.account.busy = Some("Reading the cookie file".into());
    app.be.blocking(
        move || read_cookie_file(&path),
        |app, r, _| {
            app.account.busy = None;
            match r {
                Ok(text) => send(
                    app,
                    "import_cookie_file",
                    json!({ "text": text }),
                    "Saving the session",
                    "Signed in from the cookie file",
                ),
                Err(e) => app.toast(ToastKind::Error, e),
            }
        },
    );
}

/// Reads a cookies.txt, refusing anything the engine would reject anyway.
pub fn read_cookie_file(path: &std::path::Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot open the file: {e}"))?;
    if !meta.is_file() {
        return Err("That is not a file".into());
    }
    if meta.len() > MAX_COOKIE_BYTES {
        return Err("That file is larger than 2 MiB, so it is not a cookie file".into());
    }
    std::fs::read_to_string(path).map_err(|e| format!("Cannot read the file as text: {e}"))
}

fn use_pasted_header(app: &mut App) {
    let header = app.account.header.text.trim().to_owned();
    app.account.header.clear();
    match clean_header(&header) {
        Some(h) => send(
            app,
            "import_cookies",
            json!({ "header": h }),
            "Saving the session",
            "Signed in with the pasted header",
        ),
        None => app.toast(
            ToastKind::Error,
            "Paste a Cookie header such as SID=...; HSID=...",
        ),
    }
}

/// A header with the optional `Cookie:` prefix removed, or None when it has no `name=value` pair.
pub fn clean_header(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let body = raw
        .strip_prefix("Cookie:")
        .or_else(|| raw.strip_prefix("cookie:"))
        .unwrap_or(raw)
        .trim();
    body.split(';')
        .any(|p| {
            p.trim()
                .split_once('=')
                .is_some_and(|(n, _)| !n.trim().is_empty())
        })
        .then(|| body.to_owned())
}

fn use_terminal_session(app: &mut App) {
    if app.account.busy.is_some() {
        return;
    }
    app.account.busy = Some("Reading the terminal app's session".into());
    let paths = app.be.paths.shared.clone();
    let mode = app.config.auth_store.clone();
    app.be.blocking(
        move || {
            vidcue_services::auth::SessionStore::new(&paths, &mode)
                .load()
                .map_err(|e| vidcue_providers::diagnostic(&format!("{e:#}")))
                .and_then(|s| {
                    s.map(|s| serde_json::to_value(&s.cookies).map_err(|e| e.to_string()))
                        .transpose()
                })
        },
        |app, r, _| {
            app.account.busy = None;
            match r {
                Ok(Some(cookies)) => send(
                    app,
                    "import_cookies",
                    json!({ "cookies": cookies }),
                    "Saving the session",
                    "Signed in with the terminal app's session",
                ),
                Ok(None) => app.toast(ToastKind::Info, "The terminal app has no stored session"),
                Err(e) => app.toast(
                    ToastKind::Error,
                    format!("Could not read the terminal session: {e}"),
                ),
            }
        },
    );
}

fn set_index(app: &mut App, i: u32) {
    let i = i.min(MAX_ACCOUNT_INDEX);
    if i != app.config.account_index {
        app.edit_config(|c| c.account_index = i);
        app.feeds.pages.clear();
        app.toast(ToastKind::Info, format!("Using Google account slot {i}"));
    }
}

pub fn browser_label(id: &str) -> String {
    match id {
        "brave-origin" => "Brave Origin".into(),
        "brave" => "Brave".into(),
        "chrome" => "Chrome".into(),
        "chromium" => "Chromium".into(),
        "firefox" => "Firefox".into(),
        "edge" => "Edge".into(),
        "vivaldi" => "Vivaldi".into(),
        "opera" => "Opera".into(),
        other => {
            let mut c = other.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        }
    }
}

/// The profile folder's own name (`Default`, `Profile 2`), which tells two profiles apart.
pub fn profile_name(path: &std::path::Path) -> String {
    let mut p = path;
    while let Some(n) = p.file_name().and_then(|n| n.to_str()) {
        if !matches!(n, "Cookies" | "cookies.sqlite" | "Network") {
            return n.to_owned();
        }
        match p.parent() {
            Some(q) => p = q,
            None => break,
        }
    }
    String::new()
}

// ---- view ----

pub fn view(app: &App, _cx: &mut Cx<App>) -> El {
    let t = &app.theme;
    let a = &app.account;
    let busy = a.busy.is_some();

    // Without an importable browser, signing in on Google's page is the obvious way in.
    let no_browser = a.profiles_loaded && a.profiles.is_empty();
    let mut cards: Vec<El> = vec![
        browser_card(app, busy),
        file_card(t, busy),
        header_card(app, busy),
        terminal_card(t, busy),
        session_card(app, busy),
    ];
    cards.insert(usize::from(!no_browser), google_card(t, busy, no_browser));
    let n = cards.len();
    let stagger = Stagger::new(0.05);
    let cards = cards.into_iter().enumerate().map(|(i, c)| {
        c.transition(Transition::offset(0.0, 18.0).combined(Transition::opacity()))
            .enter_delay(stagger.delay(i, n))
    });

    col().key("account-page").w_full().h_full().scroll(ScrollSpec { stick_bottom: false }).child(
        col()
            .w_full()
            .max_w(920.0)
            .px(sz::PAGE_PAD)
            .pt(sz::TOPBAR + 24.0)
            .pb(sz::DOCK + 48.0)
            .gap(16.0)
            .child(hero(app))
            .child(
                text("Signing in is optional. Without it you can still search, watch and listen; with it you get your own Home, Subscriptions and playlists.")
                    .font(ty::BODY)
                    .color(t.muted)
                    .wrap(),
            )
            .children(cards),
    )
}

fn hero(app: &App) -> El {
    let t = &app.theme;
    let a = &app.account;
    let connected = a.connected;
    let r = 24.0;

    let avatar = el()
        .key("acct-avatar")
        .size(72.0, 72.0)
        .radius(36.0)
        .fill(if connected {
            t.accent
        } else {
            Color::WHITE.alpha(0.16)
        })
        .center()
        .motion(motion::POP)
        .child(if connected { fx_glow() } else { el() })
        .child(
            icon(if connected { &ic::CHECK } else { &ic::USER }, 34.0).color(if connected {
                t.on_accent
            } else {
                Color::WHITE
            }),
        );

    let (title, sub) = if connected {
        (
            "You're signed in".to_owned(),
            a.notice
                .clone()
                .unwrap_or_else(|| "Your YouTube account is connected on this computer.".into()),
        )
    } else {
        (
            "Not signed in".to_owned(),
            a.notice.clone().unwrap_or_else(|| {
                "Browsing anonymously. Connect an account to see your own feeds.".into()
            }),
        )
    };
    let mut words = col()
        .grow(1.0)
        .shrink()
        .gap(4.0)
        .child(text(title).font(ty::H1).color(Color::WHITE))
        .child(
            text(sub)
                .font(ty::BODY)
                .color(Color::WHITE.alpha(0.78))
                .wrap(),
        );
    if let Some(b) = &a.busy {
        words = words.child(
            row()
                .gap(8.0)
                .center_y()
                .pt(4.0)
                .child(
                    el().size(8.0, 8.0)
                        .radius(4.0)
                        .fill(Color::WHITE)
                        .cycle(crate::ui::widgets::PULSE),
                )
                .child(text(b).font(ty::SMALL_STRONG).color(Color::WHITE)),
        );
    }

    let status = row()
        .key(if connected { "acct-on" } else { "acct-off" })
        .gap(20.0)
        .padding(22.0)
        .center_y()
        .radius(sz::R_PANEL)
        .fill(Color::BLACK.alpha(0.30))
        .border(1.0, Color::WHITE.alpha(0.18))
        .transition(Transition::scale(0.96).combined(Transition::opacity()))
        .role("status")
        .label(if connected {
            "Signed in"
        } else {
            "Not signed in"
        })
        .child(avatar)
        .child(words);

    el().key("acct-hero")
        .w_full()
        .h(220.0)
        .radius(r)
        .clip_rounded(r)
        .child(
            crate::fx::mesh(if connected { 5 } else { 2 }, 0.0)
                .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(r),
        )
        .child(
            col()
                .w_full()
                .h_full()
                .padding(28.0)
                .justify_end()
                .child(status),
        )
}

fn fx_glow() -> El {
    crate::fx::glow(1, 0, 0.9, 16.0)
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .radius(36.0)
}

/// A method card: glyph, title, explanation, then its controls.
fn method(t: &Theme, key: &str, glyph: &'static rune::Icon, title: &str, body: &str) -> El {
    col()
        .key(key.to_owned())
        .w_full()
        .padding(20.0)
        .gap(14.0)
        .radius(sz::R_PANEL)
        .fill(t.surface)
        .border(1.0, t.hairline)
        .hover(t.surface.mix(t.hover, 0.5))
        .motion(motion::SNAP)
        .child(
            row()
                .gap(14.0)
                .center_y()
                .child(
                    el().size(40.0, 40.0)
                        .radius(12.0)
                        .fill(t.accent_soft)
                        .center()
                        .child(icon(glyph, 20.0).color(t.accent)),
                )
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .gap(3.0)
                        .child(text(title).font(ty::H3).color(t.text))
                        .child(text(body).font(ty::SMALL).color(t.muted).wrap()),
                ),
        )
}

fn dim(el: El, disabled: bool) -> El {
    if disabled {
        el.opacity(0.5).focusable(false)
    } else {
        el
    }
}

fn browser_card(app: &App, busy: bool) -> El {
    let t = &app.theme;
    let mut c = method(
        t,
        "m-browser",
        &ic::GLOBE,
        "Import from a browser",
        "Reads that browser's YouTube and Google cookies once, locally, only when you press Use. Nothing is read in the background and nothing leaves this computer except to YouTube.",
    );
    if app.account.profiles.is_empty() {
        let msg = if app.account.profiles_loaded {
            "No supported browser profiles were found on this computer. Use Sign in with Google above instead."
        } else {
            "Looking for browser profiles…"
        };
        c = c.child(text(msg).font(ty::SMALL).color(t.faint));
    }
    for (i, p) in app.account.profiles.iter().enumerate() {
        let profile = p.clone();
        let name = browser_label(&p.browser);
        let detail = profile_name(&p.path);
        let btn = dim(
            button(t, "Use", None, Kind::Secondary)
                .key(format!("use-profile-{i}"))
                .label(format!("Use {name} {detail}"))
                .on_press(move |s: &mut App| import_browser(s, profile.clone())),
            busy,
        );
        c = c.child(
            row()
                .key(format!("profile-{i}"))
                .h(52.0)
                .px(14.0)
                .gap(12.0)
                .center_y()
                .radius(12.0)
                .fill(t.raised)
                .child(icon(&ic::GLOBE, 18.0).color(t.muted))
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .child(text(name).font(ty::BODY_STRONG).color(t.text))
                        .child(text(detail).font(ty::SMALL).color(t.muted).truncate()),
                )
                .child(btn),
        );
    }
    c
}

fn google_card(t: &Theme, busy: bool, primary: bool) -> El {
    method(
        t,
        "m-google",
        &ic::USER,
        "Sign in with Google",
        "Opens Google's sign-in page in a separate Vidcue window. Sign in as usual; the window closes by itself and Vidcue keeps only the YouTube session.",
    )
    .child(
        row().child(dim(
            button(
                t,
                "Sign in with Google…",
                Some(&ic::USER),
                if primary { Kind::Primary } else { Kind::Secondary },
            )
            .key("sign-in-google")
            .on_press(sign_in_with_google),
            busy,
        )),
    )
}

fn file_card(t: &Theme, busy: bool) -> El {
    method(
        t,
        "m-file",
        &ic::FOLDER,
        "Cookie file",
        "Choose a Netscape-format cookies.txt exported from your browser (up to 2 MiB). It is read once and not kept.",
    )
    .child(
        row().child(dim(
            button(t, "Choose cookies.txt…", Some(&ic::FOLDER), Kind::Secondary)
                .key("pick-cookie-file")
                .on_press_tx(|_s: &mut App, tx: &mut Tx| pick_cookie_file(tx)),
            busy,
        )),
    )
}

fn header_card(app: &App, busy: bool) -> El {
    let t = &app.theme;
    let input = rune::text_input(&app.account.header, ty::BODY)
        .single_line()
        .placeholder("SID=…; HSID=…; SAPISID=…", t.faint)
        .on_edit(|s: &mut App| &mut s.account.header)
        .on_submit(|s: &mut App, _tx: &mut Tx| use_pasted_header(s))
        .label("Cookie header")
        .color(t.text)
        .py(10.0)
        .px(14.0)
        .grow(1.0)
        .radius(10.0)
        .fill(t.raised)
        .border(1.0, t.border);
    method(t, "m-header", &ic::KEY, "Paste a Cookie header", "Copy the Cookie request header of a signed-in youtube.com request from your browser's developer tools. The field hides its content and is cleared once used.").child(
        row().gap(10.0).center_y().child(input).child(dim(button(t, "Sign in", None, Kind::Primary).key("use-header").on_press(|s: &mut App| use_pasted_header(s)), busy)),
    )
}

fn terminal_card(t: &Theme, busy: bool) -> El {
    method(t, "m-terminal", &ic::LINK, "Use the terminal app's session", "If you already signed in with the Vidcue or ytui terminal app, bring that session over. It is read from its own store once, when you press the button.").child(
        row().child(dim(button(t, "Use terminal session", None, Kind::Secondary).key("use-terminal").on_press(|s: &mut App| use_terminal_session(s)), busy)),
    )
}

fn session_card(app: &App, busy: bool) -> El {
    let t = &app.theme;
    let idx = app.config.account_index;
    let step = |key: &str, glyph: &'static rune::Icon, label: &str, to: u32, enabled: bool| {
        let b = icon_button(t, glyph, label, 32.0)
            .key(key.to_owned())
            .fill(t.raised)
            .on_press(move |s: &mut App| set_index(s, to));
        if enabled {
            b
        } else {
            b.opacity(0.4).focusable(false)
        }
    };
    let stepper = row()
        .gap(8.0)
        .center_y()
        .child(step(
            "acct-minus",
            &ic::DOWN,
            "Previous account slot",
            idx.saturating_sub(1),
            idx > 0,
        ))
        .child(
            text(idx.to_string())
                .font(TextStyle::new(16.0, 650.0).features("'tnum' 1"))
                .color(t.text)
                .w(24.0)
                .align(rune::Align::Center),
        )
        .child(step(
            "acct-plus",
            &ic::UP,
            "Next account slot",
            idx + 1,
            idx < MAX_ACCOUNT_INDEX,
        ));

    let mut actions = row().gap(10.0).center_y().child(dim(
        button(t, "Reload", Some(&ic::REFRESH), Kind::Secondary)
            .key("acct-reload")
            .on_press(|s: &mut App| {
                send(
                    s,
                    "reload_account",
                    json!({}),
                    "Reloading",
                    "Account reloaded",
                )
            }),
        busy,
    ));
    if app.account.confirm_disconnect {
        actions = actions
            .child(
                text("Remove the saved session?")
                    .font(ty::SMALL_STRONG)
                    .color(t.error),
            )
            .child(
                button(t, "Disconnect", Some(&ic::LOGOUT), Kind::Danger)
                    .key("acct-disconnect-yes")
                    .on_press(|s: &mut App| {
                        send(s, "disconnect", json!({}), "Disconnecting", "Signed out")
                    }),
            )
            .child(
                button(t, "Cancel", None, Kind::Ghost)
                    .key("acct-disconnect-no")
                    .on_press(|s: &mut App| s.account.confirm_disconnect = false),
            );
    } else if app.account.connected {
        actions = actions.child(
            button(t, "Disconnect", Some(&ic::LOGOUT), Kind::Danger)
                .key("acct-disconnect")
                .on_press(|s: &mut App| s.account.confirm_disconnect = true),
        );
    }

    let root = app.be.paths.root.display().to_string();
    method(t, "m-session", &ic::SETTINGS, "Session", "Reload re-reads the saved session. Disconnect removes it from this computer; saved media and downloads are kept.")
        .child(
            row().gap(14.0).center_y().child(col().grow(1.0).gap(2.0).child(text("Google account slot").font(ty::BODY_STRONG).color(t.text)).child(
                text("Which signed-in Google account to use when your browser had several. 0 is the first.").font(ty::SMALL).color(t.muted).wrap(),
            )).child(stepper),
        )
        .child(actions)
        .child(
            col().gap(4.0).padding(14.0).radius(12.0).fill(t.raised)
                .child(text("Where your session is kept").font(ty::SMALL_STRONG).color(t.text))
                .child(text(format!("The session is saved in a private file, account.json, inside {root} (a folder only your user can open). It is not put in the system keyring, and the signed-in cookies are only ever sent to YouTube.")).font(ty::SMALL).color(t.muted).wrap()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_accepts_optional_prefix() {
        assert_eq!(
            clean_header("Cookie: SID=a; HSID=b").as_deref(),
            Some("SID=a; HSID=b")
        );
        assert_eq!(clean_header("  SID=a ").as_deref(), Some("SID=a"));
        assert_eq!(clean_header("nothing here"), None);
        assert_eq!(clean_header("=x"), None);
        assert_eq!(clean_header(""), None);
    }

    #[test]
    fn labels_and_profile_names() {
        assert_eq!(browser_label("brave-origin"), "Brave Origin");
        assert_eq!(browser_label("zen"), "Zen");
        assert_eq!(
            profile_name(std::path::Path::new("/x/Brave/Default")),
            "Default"
        );
        assert_eq!(
            profile_name(std::path::Path::new("/x/Brave/Profile 2/Network/Cookies")),
            "Profile 2"
        );
    }

    #[test]
    fn cookie_file_limits() {
        let dir = std::env::temp_dir().join(format!("vc-acct-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("ok.txt");
        std::fs::write(&ok, "# Netscape HTTP Cookie File\n").unwrap();
        assert!(read_cookie_file(&ok).is_ok());
        let big = dir.join("big.txt");
        std::fs::write(&big, vec![b'a'; (MAX_COOKIE_BYTES + 1) as usize]).unwrap();
        assert!(read_cookie_file(&big).is_err());
        assert!(read_cookie_file(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
