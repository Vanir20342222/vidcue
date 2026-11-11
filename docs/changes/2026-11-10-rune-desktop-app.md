# Rune desktop app with Android parity and Cinecat movies alpha (M9)

- **Date:** 2026-11-10
- **PR:** none yet
- **Milestone:** M9 (monorepo, shared engine; the desktop GUI the M9 notes anticipate)
- **Platforms:** desktop (Linux; Omarchy target)
- **Status:** fixture/unit verified; embedded playback and one search/resolve/play path live verified on the target PC

## What it does
Adds `apps/desktop` (`vidcue-desktop`), a GPU-rendered desktop app built with Rune
(`anomalyco/rune`, pinned rev `7ed827a2`). It drives the same `vidcue_mobile::Engine` as Android, so
these behave as on Android:
- Home, Music and Radio feeds with chips and continuation
- search and pasted links
- collections and channels
- queue with context continuation (playlist tail, related, nearby stations)
- history and resume, saved items, offline playlists
- lyrics (local, YouTube Music, LRCLIB, LRC import, offset)
- account cookie import and settings

Video plays inside the window: libmpv's software renderer feeds a Rune `NativeSurface`, with an
ambient-light blur behind it, app-drawn captions, SponsorBlock marks, chapters, and quality, audio
track and caption menus. The player also has a sleep timer, output devices, an equalizer and colour
grades.

The app also has:
- the four music player styles (classic, vinyl, VU, orbit)
- downloads through `vidcue_services::download`
- hover previews
- the 14 palettes, a live "follow desktop" theme (Omarchy/Caelestia/portal) and palette import
- onboarding and update checks
- an opt-in **Movies (alpha)** section on the Cinecat backend: curated TMDB pool, optional TMDB key
  for search and posters, verified failover resolve

It uses Rune's effects:
- backdrop blur, shared-element (`matched`) flights, presence transitions, springs
- live values for seek bars and meters
- a rune-fx compositor: mesh gradients, aurora, glow, pulse, sparkles, liquid metal, liquid-glass
  lens, shimmer, grain

Decorative effects hold a still pose so idle pages stop drawing.

State lives in `~/.local/state/vidcue/desktop` and `~/.cache/vidcue/desktop`. On first start, the
engine settings are seeded from the terminal app's `rust.toml`; that file is never written.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Workspace format | `cargo fmt --all -- --check` | clean | unit |
| Workspace lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean | unit |
| Workspace tests | `cargo test --workspace --locked` | all suites pass; desktop 75 passed, 2 ignored (live) | unit/fixture |
| Headless UI interaction | `ui_tests.rs`: Rune `Runtime` with real pointer/key input, app `poll`/`on_key`, isolated offline engine | navigation, chips, typed search, pasted link, player menu speed, shortcuts, card menu save + queue through the engine, theme persisted by the engine, onboarding, downloads pause/resume, music styles | fixture |
| Accessibility names | every scene: every button has a label or text | pass (found and fixed 6 unnamed controls) | fixture |
| Embedded player | generated FFmpeg clip → libmpv sw render → `NativeSurface` | frames delivered, 640×360, 2.0 s, EOF; nvdec-copy | fixture (real libmpv) |
| Live playback | `cargo test -p vidcue-desktop live_ -- --ignored` | YouTube search → native resolve 720p → 60 frames in ~3 s; radio station plays with ICY title | live verified (one video, one station, this network) |
| Headless renders | `vidcue-desktop --snap … --scene <home/watch/song-*/downloads/settings/movies/…>` and scripted `--script` flows | reviewed visually; idle pages at rest after transitions | fixture |
| Windowed start | isolated `XDG_*` dirs, 25 s run | no panic; first frame 3.0 s (debug); engine `settings/saved/account/up_next` and live `browse` ok | live (startup only) |
| Windows cross-compile | `cargo check --target x86_64-pc-windows-gnu -p vidcue-desktop --locked` with a local mingw header set (`DOCS_RS=1` skips libesedb-sys's host-OS C build) | compiles clean after gating the Unix-only `player`/`mpris` modules and adding a Windows `open_file` (linking and the bundled-DLL playback happen in the `windows-release` workflow) | unit |
| Windows packaging | `.github/workflows/windows-release.yml` on `desktop-*` tags; `tools/windows/vidcue.iss` | run green on `desktop-0.2.0-alpha.13`; `VidcueSetup-0.2.0-alpha.13.exe` and `Vidcue-portable.zip` attached; the exe imports only Windows system DLLs plus the bundled `libmpv-2.dll` and uses the GUI subsystem | CI |
| Windows exe under Wine | release `vidcue.exe --snap home.png --scene home` in a throwaway Wine 11.20 prefix with native `d3dcompiler_47` (Rune's DX12/FXC path; Wine's built-in FXC lacks SM 5.1 resource arrays) | home scene rendered correctly | emulated (Wine, headless) |
| System install | `~/.local/bin/vidcue` + hicolor icon + `dev.vidcue.desktop.desktop` entry (same layout `install.sh` writes); app running from the installed path | installed and launched on the target PC | live (target PC) |
| Update-check release | GitHub release `desktop-0.2.0-alpha.13` from this tag; `vidcue-services::update::newest` tags rules (`v*`/`desktop-*`, semver, pre-release gating) | the installed 0.2.0-alpha.12 build can find it via Settings → About → Check for updates | unit rules + live list |

## Not verified and limits
- **Human checks:** no human interaction or visual acceptance on Ghostty, Brave Origin or Omarchy;
  no audible-output check.
- **Accounts:** no signed-in account was used. Browser import reads cookies only on an explicit
  press. The desktop engine stores the session as the engine's private `account.json` (not the
  keyring the terminal app used).
- **Movies:** not resolved live from the UI. Cinecat availability is provider-dependent, as recorded
  in the 2026-11-09 Cinecat notes. The section is opt-in behind a notice.
- **Video rendering cost:** the software render path costs CPU (BGRA conversion and upload). It is
  capped at the drawn size and at most 1080p. Auto colour grade is not applied by the embedded
  player.
- **Downloads:** downloads run without the account session, so private items fail.
- **Dependencies and CI:** the Linux build needs `libmpv` (API 2.x), `libxkbcommon` and `wayland`.
  The desktop crate is not yet in a Linux CI workflow; a release build with those packages
  installed on the target PC is verified instead.
- **Windows packaging:** the workspace compiles for `x86_64-pc-windows-gnu` (see evidence), and
  `.github/workflows/windows-release.yml` builds an Inno Setup installer plus a portable zip on
  `desktop-*` tags, bundling the current `libmpv-2.dll` from zhongfly/mpv-winbuild. The
  workflow ran green and a headless render works under Wine, but the installer, windowed use
  and bundled-DLL playback have not run on a real Windows machine; no Windows acceptance claim
  is made. MinGW builds set `-DHAVE_WINDOWS_H` so libesedb-sys (rookie's Edge cookie reader)
  uses its WinAPI configuration. Optional yt-dlp and FFmpeg are not bundled. The
  in-app updater on Windows offers the release page only; the built-and-install updater flow
  is Linux.
- **No reliability claim:** no 99 % watching claim is made; the live checks cover one video and one
  station.

## For the plan and parity tables
- M9: A Rune desktop GUI (`apps/desktop`) drives the shared `vidcue_mobile::Engine` with Android
  feature parity for feeds, search, player controls, queue/context, history/resume, saved,
  offline playlists, lyrics, music styles, downloads, themes, account import, settings and
  onboarding; embedded libmpv playback; opt-in Cinecat Movies alpha. Fixture/unit verified;
  one live search→resolve→play and one radio station verified on the target PC; human desktop
  acceptance open.
