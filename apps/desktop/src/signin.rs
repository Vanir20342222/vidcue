//! "Sign in with Google": Google's own sign-in page in a small window of the system web view
//! (WebView2 on Windows, WebKitGTK on Linux), for people without a browser Vidcue can import.
//!
//! The window runs in a separate process (`vidcue __signin <dir>`), because the web view needs
//! its own event loop next to Rune's. It uses a fresh web profile in `<dir>/profile`, so nothing
//! from another browser is read. Once YouTube's sign-in cookies appear, it writes the YouTube
//! `Cookie` header to `<dir>/header` and exits 0; the app imports it exactly like a pasted
//! header and deletes `<dir>`. Closing the window exits 1 and imports nothing.

use std::path::Path;
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::{PageLoadEvent, WebContext, WebViewBuilder};

/// Google's sign-in page, returning to YouTube (the same entry point the Android app uses).
pub const SIGN_IN_URL: &str = "https://accounts.google.com/ServiceLogin?service=youtube&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26next%3D%252F";

/// The process argument that runs the sign-in window instead of the app.
pub const ARG: &str = "__signin";

/// A YouTube cookie header from a signed-in session, as the Android app recognises it.
pub fn looks_signed_in(header: &str) -> bool {
    header.split(';').any(|pair| {
        let name = pair.split('=').next().unwrap_or_default().trim();
        matches!(name, "SAPISID" | "__Secure-3PAPISID" | "LOGIN_INFO")
    })
}

/// Opens the window and blocks until the user signs in (exit code 0) or closes it (1).
pub fn run(dir: &Path) -> i32 {
    match open(dir) {
        Ok(never) => match never {},
        Err(e) => {
            eprintln!("Sign-in window failed: {e:#}");
            2
        }
    }
}

enum Never {}

fn open(dir: &Path) -> anyhow::Result<Never> {
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("Sign in to YouTube · Vidcue")
        .with_inner_size(LogicalSize::new(520.0, 760.0))
        .with_min_inner_size(LogicalSize::new(380.0, 520.0))
        .with_window_icon(icon())
        .build(&event_loop)?;

    // A throwaway web profile: the sign-in is not kept anywhere except in Vidcue's session.
    // The app deletes it once this process (and the web view's helpers) have exited.
    let mut context = WebContext::new(Some(dir.join("profile")));
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(SIGN_IN_URL)
        .with_on_page_load_handler(move |event, _url| {
            if matches!(event, PageLoadEvent::Finished) {
                let _ = proxy.send_event(());
            }
        });
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window)?;
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| anyhow::anyhow!("The sign-in window has no GTK container"))?;
        builder.build_gtk(vbox)?
    };

    let out = dir.join("header");
    event_loop.run(move |event, _, control_flow| {
        // Page-load events can miss in-page navigations, so also look once a second.
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(1));
        let check = matches!(
            event,
            Event::UserEvent(())
                | Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. })
        );
        if check && let Some(header) = signed_in_header(&webview) {
            let saved = vidcue_providers::paths::atomic_write(&out, header.as_bytes()).is_ok();
            *control_flow = ControlFlow::ExitWithCode(if saved { 0 } else { 2 });
        }
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::ExitWithCode(1);
        }
    })
}

/// The YouTube cookie header once the page is back on YouTube with a signed-in session.
fn signed_in_header(webview: &wry::WebView) -> Option<String> {
    let url = webview.url().ok()?;
    let host = url::Url::parse(&url).ok()?.host_str()?.to_owned();
    if host != "youtube.com" && !host.ends_with(".youtube.com") {
        return None;
    }
    let header = webview
        .cookies_for_url("https://www.youtube.com/")
        .ok()?
        .iter()
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; ");
    looks_signed_in(&header).then_some(header)
}

fn icon() -> Option<Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(include_bytes!(
        "../assets/vidcue-256.png"
    )));
    let mut reader = decoder.read_info().ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgba).ok()?;
    (info.color_type == png::ColorType::Rgba)
        .then(|| Icon::from_rgba(rgba, info.width, info.height).ok())
        .flatten()
}

/// Runs the sign-in window as a child process and returns the captured cookie header.
/// Blocks until the window closes; call it off the UI thread.
pub fn sign_in() -> Result<String, String> {
    let dir = tempfile::Builder::new()
        .prefix("vidcue-signin-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .arg(ARG)
        .arg(dir.path())
        .stdin(std::process::Stdio::null());
    // WebKitGTK's DMA-BUF renderer dies with a Wayland protocol error on some drivers
    // (seen with NVIDIA on Hyprland); the shared-memory path works everywhere.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        command.env("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    let status = command.status();
    let header = std::fs::read_to_string(dir.path().join("header"));
    remove_soon(dir);
    let status = status.map_err(|e| format!("Cannot open the sign-in window: {e}"))?;
    match status.code() {
        Some(0) => header.map_err(|e| e.to_string()),
        Some(1) => Err("Sign-in was cancelled".into()),
        _ => Err(if cfg!(windows) {
            "The sign-in window could not start. It needs Microsoft Edge WebView2, which Windows 10 and 11 normally include".into()
        } else {
            "The sign-in window could not start. It needs WebKitGTK 4.1 (webkit2gtk-4.1)".into()
        }),
    }
}

/// Deletes the sign-in folder (header and web profile). WebView2's helper processes can hold
/// the profile for a moment after the window closes, so a locked folder is retried briefly.
fn remove_soon(dir: tempfile::TempDir) {
    let path = dir.keep();
    for _ in 0..20 {
        match std::fs::remove_dir_all(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                std::thread::sleep(Duration::from_millis(250));
            }
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::looks_signed_in;

    #[test]
    fn signed_in_cookies_match_the_android_rule() {
        assert!(looks_signed_in("PREF=f6=1; SAPISID=abc; YSC=x"));
        assert!(looks_signed_in("__Secure-3PAPISID=abc"));
        assert!(looks_signed_in(" LOGIN_INFO=AFm"));
        assert!(!looks_signed_in("PREF=f6=1; YSC=x; VISITOR_INFO1_LIVE=y"));
        assert!(!looks_signed_in(""));
    }
}
