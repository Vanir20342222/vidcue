# Vidcue desktop (Rune)

A GPU-rendered desktop app for Vidcue, built with [Rune](https://github.com/anomalyco/rune)
(Rust, wgpu/Vulkan). It drives the same shared engine as the Android app
(`vidcue_mobile::Engine`), so browsing, queue, context continuation, history, resume, saved
items, offline playlists, lyrics, account and settings behave the same way on both.

## Run

Linux needs `libmpv` (mpv 0.38 or newer, API 2.x), `libxkbcommon`, `wayland` and WebKitGTK 4.1
(`webkit2gtk-4.1`, for the "Sign in with Google" window) at build time. On Windows the sign-in
window uses the Microsoft Edge WebView2 runtime that ships with Windows 10 and 11.
Node.js 22+ or Deno 2.9.7 must be installed for YouTube's JavaScript challenges; the app
detects them and never installs anything.

```sh
cargo run --release -p vidcue-desktop
```

Headless renders use synthetic, offline scenes (no network, no user files):

```sh
target/release/vidcue-desktop --list-scenes
target/release/vidcue-desktop --snap home.png --scene home --size 1440x900
```

## How it is put together

| Area | Where |
| --- | --- |
| Engine bridge: Tokio runtime, engine calls landing on the UI thread | `src/backend.rs` |
| State, routes, desktop preferences, toasts | `src/app.rs` |
| Embedded player: libmpv's software renderer → Rune `NativeSurface` | `src/player/mpv.rs`, `src/player/ffi.rs` |
| Playback session: resolve, queue/context, history, resume, SponsorBlock, sleep timer, renewal | `src/player/session.rs` |
| Hover previews (second, small mpv) | `src/player/preview.rs` |
| Downloads (shared `vidcue_services::download` worker) | `src/downloads.rs` |
| Cinecat movies (alpha) | `src/movies.rs`, `src/ui/movies.rs` |
| GPU effects compositor (rune-fx) | `src/fx.rs` |
| Screens | `src/ui/` |
| Headless scenes | `src/scenes.rs` |

State lives in `~/.local/state/vidcue/desktop` (engine `config.json`, `library.json`,
`lyrics.json`, the account session, `downloads.json`, `desktop.json`) and
`~/.cache/vidcue/desktop`. On the first start the engine settings are seeded from the
terminal app's `~/.config/vidcue/rust.toml` when it exists; that file is never written. The
ytui → vidcue folder migration in `vidcue_providers::paths` still runs first.

### Video in the window

Rune's own `Player` plays video on macOS only. On Linux the app embeds libmpv: mpv decodes
(hardware decode with copy-back where available), keeps A/V sync and plays audio; its software
renderer converts each frame to BGRA at no more than the drawn size (and at most 1080p) on a
render thread and pushes it to a `NativeSurface`. The GPU scales and composites it like any
element, so video takes part in rounded clips, the ambient-light blur behind the player,
shared-element flights to the dock, and captions drawn by the app over the picture.
Position, levels and the seek bars are Rune `Live` values and `LiveText`, which redraw
without re-running the view.

### Effects

Frosted glass (`backdrop_blur`) for the top bar, dock, menus and toasts; shared-element
motion (`matched`) for artwork flying from cards into the player and for the sidebar
selection; presence transitions for pages, toasts and lists; springs and press-scale on every
control; and rune-fx layers through a compositor (`src/fx.rs`): palette mesh gradients, aurora,
glow, pulse, sparkle bursts, film grain and liquid metal. Effects follow reduced motion and the
"GPU effects" preference, and the window only keeps drawing while an animated layer is visible.

## Checks

```sh
cargo test -p vidcue-desktop                      # offline: unit, headless UI interaction, embedded-player clip
cargo test -p vidcue-desktop live_ -- --ignored   # opt-in live: YouTube search→resolve→play, a radio station
VIDCUE_DESKTOP_TRACE=1 vidcue-desktop             # log engine op names and outcomes (no URLs or secrets)
```
