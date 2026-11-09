# Vidcue for Android

**Vidcue** is a native Android app for watching and listening to **YouTube**, **YouTube Music** and **internet radio**, with background playback and offline downloads. The UI is Jetpack Compose; media playback uses Media3; the network, catalog and extraction engine is shared Rust (`crates/vidcue-mobile` compiled through JNI). No Python and no mpv executable runs on Android, and the explicit account sign-in page is the app's only WebView.

This repository is the Android app plus the shared Rust backend crates it builds on. A future desktop application will reuse the same backend; the old terminal UI has been removed.

> [!NOTE]
> **ytui is now Vidcue.** Older documentation and release notes may still say ytui. The
> canonical repository is `Vanir20342222/vidcue`; the [Vanir20342222/ytui](https://github.com/Vanir20342222/ytui)
> repository only mirrors its releases so older installs and links keep working.

## Install

Download a signed APK from the [releases page](https://github.com/Vanir20342222/vidcue/releases), open it in Android Files and allow installation from that source. The current beta channel installs the permanent package `dev.vidcue.android.stable` (Android 8 / API 26+, ARM64 and x86-64); later betas and final releases update that installation in place. Debug CI artifacts are the separate **Vidcue Preview** app.

Details, release qualification records and the feature walkthrough live in [`docs/android/README.md`](docs/android/README.md).

## Screens

Actual emulator captures of the current beta (see [docs/screenshots/android/2026-11-08/](docs/screenshots/android/2026-11-08/README.md)):

<p align="center">
  <img src="docs/screenshots/android/2026-11-08/native-catalog.png" alt="Home feed, dark theme" width="26%">
  &nbsp;
  <img src="docs/screenshots/android/2026-11-08/native-music.png" alt="Music grid" width="26%">
  &nbsp;
  <img src="docs/screenshots/android/2026-11-08/native-player.png" alt="Focused player" width="26%">
</p>

<p align="center">
  <img src="docs/screenshots/android/2026-11-08/native-settings.png" alt="Appearance settings" width="26%">
  &nbsp;
  <img src="docs/screenshots/android/2026-11-08/restyle-fidelity-phone-queue-populated.png" alt="Playback queue" width="26%">
  &nbsp;
  <img src="docs/screenshots/android/2026-11-08/restyle-phone-light-vidcue-light-feed.png" alt="Light theme" width="26%">
</p>

<p align="center">
  <img src="docs/screenshots/android/2026-11-08/native-wide-catalog.png" alt="Tablet catalog with navigation rail" width="80%">
</p>

## Build from source

Requirements: Rust (see `rust-toolchain.toml`), the [Android SDK](https://developer.android.com/studio) with `ANDROID_HOME` set, and NDK `27.2.12479018` (or point `ANDROID_NDK_HOME` at an installed NDK).

```sh
tools/android/build.sh            # debug APK
YTUI_ANDROID_VARIANT=release tools/android/build.sh   # needs signing env; see docs/android/SIGNING.md
```

The script builds the Rust engine for each ABI, then packages the Gradle app. It installs nothing on the host or on a device.

## Repository layout

| Path | What it is |
|---|---|
| `apps/android` | Native Jetpack Compose Android app (Kotlin/Java UI, activity, media and storage adapters) |
| `crates/vidcue-mobile` | Android's Rust engine: JNI bridge, native playback and catalog worker |
| `crates/vidcue-providers` | Shared YouTube/Music/radio provider and extraction logic |
| `crates/vidcue-services` | Shared backend services: playback state, downloads, library, storage |
| `crates/vidcue-core` | Shared data model: media, queue, history, settings |
| `vendor/yt-dlp-rs` | Patched vendored extractor used by the native engine |
| `tools/android` | Android build, signing, release and device-test tooling |
| `tools/ci` | CI path filters and PR checks |
| `docs/android` | Android plan, parity gates, validation and release records |

## Status

This is an early beta: real-network latency, audio output on more devices, physical-device in-place updates and the remaining [parity gates](docs/android/PARITY.md) are still open. Current validation evidence is recorded in [`docs/android/VALIDATION.md`](docs/android/VALIDATION.md) and [`docs/changes/`](docs/changes/).

## Legal notice and disclaimer

Vidcue is an independent, open-source media client. It is **not affiliated with, endorsed by, sponsored by, or connected to YouTube, Google LLC, YouTube Music, or any other streaming platform or rights holder**. "YouTube" and "YouTube Music" are trademarks of their respective owners; they are used here only to describe interoperability, not to imply endorsement.

By using Vidcue you acknowledge and agree that:

- **You are responsible for how you use the app.** Vidcue is a general-purpose tool. You must use it in compliance with the terms of service of any platform you access, applicable copyright law, and the laws of your jurisdiction. Accessing or downloading content that you are not authorized to access, or redistributing content you do not own the rights to, may be unlawful. The app's developers do not monitor, control, or endorse any particular use.
- **Downloads are for personal, lawful use.** Offline download features are intended for content you own, content that is licensed for download, or other use permitted by law. You may not use Vidcue to pirate, redistribute, or commercially exploit content.
- **Accounts are yours alone.** The explicit account sign-in uses your own credentials directly with the platform. Vidcue's developers never see, collect, or store your credentials or session data on any server; authentication data stays on your device.
- **No warranty.** Vidcue is provided under the MIT license, without warranty of any kind. Platform changes, terms-of-service enforcement, or regional restrictions may cause features to stop working at any time.
- **Limitation of liability.** To the maximum extent permitted by law, the authors and contributors are not liable for any claims, damages, or other liability arising from the use or misuse of the app, including any violation of third-party terms of service or law by a user.

If you believe content made available through a feed or playlist served by this project infringes your rights, contact the repository owner via the issue tracker for takedown requests.

This notice supplements, and does not replace, the [MIT License](LICENSE) below.

## License

MIT — see [LICENSE](LICENSE). Third-party notices are generated at build time (`crates/vidcue-mobile/examples/mobile_license_bundle.rs`) and bundled in the app.
