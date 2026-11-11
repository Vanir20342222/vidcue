# Android beta (M9)

The app combines the shared Rust engine with a native Jetpack Compose phone/tablet UI. Media3 handles video, background playback, media controls and offline transfers. The explicit account sign-in page is its only WebView. No Python or mpv executable runs on Android.

[Android downloads and checksums](https://github.com/Vanir20342222/ytui/releases) · [Beta.1 channel and publication](BETA1_RELEASE.md) · [Actual gallery](../screenshots/android/README.md)

## Documents

Current guides: [UX overhaul](UX_OVERHAUL.md) and [UI fidelity review](UI_FIDELITY_REVIEW.md)
(the Home tab, watch page, Downloads and search-focus work now on `main`), [plan](PLAN.md),
[parity](PARITY.md), [validation record](VALIDATION.md), [signing and updates](SIGNING.md)
and the [device checklist](DEVICE_ACCEPTANCE.md).

Releases and qualification, newest first: [beta.1](BETA1_RELEASE.md), [mobile.10](MOBILE10_RELEASE.md),
[mobile.9](MOBILE9_RELEASE.md), [player/library qualification (mobile.4)](PLAYER_LIBRARY_VALIDATION_2026-10-29.md).

Design history and reviews: [content-first restyle](CONTENT_FIRST_RESTYLE.md),
[UI PR #27 review](UI_PR27_REVIEW.md), [media UI redesign](MEDIA_UI_REDESIGN.md),
[native UI migration](NATIVE_UI_MIGRATION.md), [player/library plan](PLAYER_LIBRARY_PLAN.md)
and [radio artwork](RADIO_ARTWORK.md).

Startup latency: [two-second target](TWO_SECOND_STARTUP.md), [mobile.5 validation](STARTUP_VALIDATION_2026-10-29.md),
[mobile.6](MOBILE6_STARTUP_FOLLOWUP.md), [mobile.7](MOBILE7_STARTUP_FOLLOWUP.md) and
[mobile.8](MOBILE8_INDEXED_STARTUP.md).

## Install an APK

Open the downloaded APK in Android Files and allow installation from that source when prompted. Beta.1 is version **0.3.0-beta.1**, code **30001**, with ARM64 and x86-64 support, Android 8/API 26+ minimum and API 35 target.

Open **Vidcue** (`dev.vidcue.android.stable`). Beta.1 starts a separate installation alongside old alphas and Preview apps, with its own account/settings/library state. Every later beta, RC and final release uses the same permanent package/key and updates that beta in place. About → Check for updates offers future signed releases. [Signing and recovery](SIGNING.md) describe the private backup; no GitHub signing secrets are needed. Debug CI artifacts are the separate **Vidcue Preview** app.

This beta remains an early testing release. Real YouTube latency, audible output, physical-device in-place updates and [parity/device gates](PARITY.md) remain open. The [older mobile.10 qualification](MOBILE10_RELEASE.md) belongs to that historical alpha, not to a new physical beta acceptance result. See the [device checklist](DEVICE_ACCEPTANCE.md).

## Navigate

First installations default to 1080p with H.264/AAC preference for mobile decoding
and portable file export; audio-only playback still selects the best audio.
Existing quality choices are preserved. Choose Best explicitly for unrestricted formats.
Use the bottom navigation for Home, Music, Radio and Downloads (the first tab is labelled YouTube in mobile.10 and earlier; Subscriptions and Watch later are Home filter chips on current `main`). Search accepts words
or a media URL. **Browse** selects provider shelves; categories and pagination use
server responses. The top **⋮** menu, **sliders** (Settings) and **profile** (Account) icons provide touch controls.
Tap artwork to play/open it. The card's **⋮** menu contains preview, queue, save
and download actions. Downloads → Add URL
accepts a video or dedicated playlist link, with audio-only and quality choices.
A watch link with `list=` keeps single-video intent. Tap the current player
to open transport/volume/speed, quality, audio/caption tracks, SponsorBlock, queue
and visualizer controls. Tablet layouts use a navigation rail and keep the catalog and active player side by side.
Android Back closes overlays, then returns to the previous feed with its scroll
position, before putting the app in the background. Each tab retains up to twelve
navigation steps and 2 MiB of cached previous pages. Disconnecting clears account navigation history. The queue's
arrow buttons reorder items; edits are saved before their result is shown.

Music albums/playlists and cliamp track libraries continue automatically after
the selected song. The full finite collection prepares in the background; manual
queue entries play first. Menu → Recently played opens local playback history.
Previous restarts a track after three seconds, otherwise returns to the prior
played item; Next follows the manual queue, forward history, then collection.
Notification/media-session controls use those same actions. History is local and
can be disabled or cleared with resume positions. A stopped/killed process does
not retain derived collection context; select the collection again to rebuild it.

Outside a collection, **Related media** prepares the next item by default; disable it under Playback. Prefetch respects the mobile-data setting. For Music/Radio, tap the mini-player for the non-scrolling classic/Vinyl/VU/Orbit screen; use its down icon or enabled swipe gesture to minimize. Advanced options open separately. Video has native controls and a landscape fullscreen icon. The sliders icon in the top bar of the video opens Playback options (volume, speed, quality, audio and subtitles, queue, sleep timer, marked segments and stop). In fullscreen it opens the same options in a side panel without leaving fullscreen.

Downloads has **Offline**, **Playlists** and **Transfers** sections. Create and edit local offline playlists there. Settings → Offline & storage chooses separate Music and Video folders through Android's folder picker; compatible completed media can save there automatically. Private streaming cache stays separate and unavailable grants retain cached media. **My playlists** chips in YouTube/Music open the account's own collections.

Menu → **Lyrics** opens a song-specific view. Review title and artist
before choosing Find on LRCLIB; no automatic lookup or cookies are sent. Choose
a returned version or import a UTF-8 LRC/plain text document (up to 512 KiB).
Enable **Follow playing line** for automatic scrolling; timed lines support tap-to-seek. Positive Delay shows lines
later; edits and adjustments save locally for that media URL. Timing pauses when
another song is playing. The local lyric library holds up to 100 songs / 8 MiB;
Remove saved lyrics frees an entry without changing saved media or the queue.

For a completed offline download, tap **Export file…** and choose a
destination in Android's document picker. Original MP4/M4A/WebM containers are
copied without re-encoding; compatible separate video/audio tracks are combined
into one MP4 or WebM file. Export runs in a foreground worker, reads only completed
offline cache data and does not contact the original server. Export jobs appear
in Downloads; Remove cancels/dismisses a job, preserving a completed exported file.
Segmented streams, live recordings and incompatible codec combinations cannot
currently be exported. MP3/FLAC conversion is not offered as an implemented option.

Appearance follows Android light/dark or selects a built-in palette. Import a
Caelestia JSON, Omarchy TOML or exported vidcue Palette JSON through the document
picker; Linux theme daemons are not accessed on a phone. Importing creates a
private copy. Vinyl/VU/Orbit are available only for Music/Radio. VU/Orbit use decoded
PCM levels, not microphone input; reduced motion and pause stop their animation.

Account access is explicit: in-app YouTube sign-in (which Google may reject in an
embedded browser), a Netscape cookie file selected by the user, or an explicitly
pasted Cookie header. Use **Sign in / Account** in the top bar, or the account
action on a protected-feed error. After signing in, choose **Use this session**;
**Cancel**, rotation or closing the page never imports implicitly. Another browser's
private database is inaccessible on normal Android. Disconnect clears the app's
session; local saved media/queue stay. Never send cookies to a bug report.

Android encrypts persisted sessions using AES-GCM and a non-exportable key in
Android Keystore. Earlier private plaintext imports migrate on load; if cleanup
fails the app reports it. New document imports parse in memory without a plaintext
temporary file. Account → Reload stored account retries the saved envelope. A
missing/invalid key or damaged envelope clears active authentication and keeps the
offline library usable; reconnect/import or disconnect to recover. Hardware key
backing depends on the device. The app never falls back to new plaintext storage.
Native extraction builds its cookie jar in memory too. Recognized stale private
cookie staging files from earlier builds are removed at startup; unrelated files
are preserved and a cleanup failure is reported.

Android uses the system VPN normally. An explicit HTTP proxy can be set in
Settings; credential-bearing/SOCKS proxies are rejected on mobile rather than
routing only part of the app. With an explicit proxy, use cookie-file import
instead of WebView sign-in. HTTP radio stations are supported; HTTPS extraction
still verifies TLS. Update Android System WebView for the isolated challenge
solver, which has no browser, cookie or network APIs.

## Build and check

Install stable Rust, JDK 21, Android command-line tools, SDK platform/build-tools
35 and NDK 27.2.12479018. Accept SDK licenses yourself. Then:

```bash
rustup target add aarch64-linux-android x86_64-linux-android
export ANDROID_HOME=/path/to/android-sdk
./tools/android/build.sh
./tools/android/inspect-apk.sh
```

The build script installs nothing. `YTUI_ANDROID_ABIS=arm64-v8a` builds only your
phone ABI for development; the distributed universal APK needs both ABIs for the
inspection gate. `YTUI_ANDROID_RUST_ONLY=1` builds native libraries without Gradle.
`YTUI_ANDROID_SKIP_GRADLE=1` also writes the license notices, then stops before Gradle.
Output: `apps/android/app/build/outputs/apk/debug/app-debug.apk`.
Native libraries go to ignored `target/android-jni`, with 16 KiB ELF segments.
The script includes application and Rust dependency license inventories. Android
libraries carry their original notices; review the complete dependency inventory
before a stable distribution.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
./tools/test-native-solver.sh # installed Node.js 22+, offline player vectors
./tools/android/test-fixtures.sh # installed FFmpeg, generated local media only
./apps/android/gradlew -p apps/android :app:testDebugUnitTest :app:lintDebug \
  :app:connectedDebugAndroidTest
```

Instrumentation needs an API 35 emulator with current Android System WebView or
an explicitly selected disposable test device. It changes that app's player,
volume, downloads and private library. Never point these tests at a personal
production install. It uses synthetic sessions and local generated media, not a
real account or public YouTube by default. For an explicit cookie-free public
check, dispatch the Android workflow with its `public_url` input, or run:

```bash
YTUI_ANDROID_PUBLIC_URL='https://www.youtube.com/watch?v=jNQXAC9IVRw' \
  ./tools/android/run-device-tests.sh
```

That additional test requires native extraction and three seconds of actual
decoding. A public-service failure fails the run; it is never replaced by a local
fixture pass. Public/provider and physical-device acceptance stay separate;
the desktop installer/tests remain unchanged in behavior.

## Repository structure

- `apps/desktop`: Rust terminal app, still `vidcue` and `vidcue`.
- `apps/android`: packaging and Android platform adapters, no application UI.
- `crates/vidcue-core`: shared models, configuration, themes and policy.
- `crates/vidcue-providers`: shared networking, YouTube/Music/radio, sessions,
  bounded caches, images, lyrics and SponsorBlock.
- `crates/vidcue-services`: desktop process, browser/keyring, mpv and OS adapters;
  re-exports shared providers for compatibility.
- `crates/vidcue-mobile`: Rust Android engine, adaptive touch UI, GLES video,
  JNI boundary and PCM visualizer metering.
- `vendor/winit`: pinned 0.30.13 with a documented Android destruction/recreation
  adapter; other platforms retain upstream behavior.
- `vendor/eframe`: pinned 0.33.3 with documented Android suspended-window and EGL
  teardown handling so application cleanup runs without a caught Rust panic.
- `vendor/yt-dlp-rs`: pinned upstream source with a documented isolated-JS runtime
  callback; desktop Node/Deno behavior is retained. See its `YTUI_PATCH.md`.

## Explicit public YouTube acceptance

Use the Android workflow's **Run workflow → public_url** input for a public video
URL, for example `https://www.youtube.com/watch?v=jNQXAC9IVRw`. This invokes the
actual Rust extractor/Android JS bridge and requires three seconds of real Media3
playback. It imports no cookies. Default CI skips that separate live check;
rate limits, account/consent requirements and runner connectivity remain failures
in an opted-in run, not evidence of working public playback.
