# Android in-app updates and a beta-safe release channel (M9)

- **Date:** 2026-11-04
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android, CI-release, docs
- **Status:** fixture/unit verified (Robolectric + Python); not emulator or physical-device verified

The user asked for this together with the beta preparation, so it touches release workflows
(AGENT_PR_RULES §2). The task prompt takes precedence over that rule here.

## What it does
- **App:** signed Vidcue builds check GitHub when they open (at most every 6 h; never in debug,
  Preview or offline-UI builds). If a newer release can replace this installation, a sheet offers
  **Update now · Later · Skip this version**.
  - **Update now** downloads the APK and checks its SHA256, package, version code and signing
    certificate. It then commits a PackageInstaller session, and Android shows its own
    confirmation screen. If Vidcue may not yet install apps, the sheet opens Android's
    "Install unknown apps" switch first.
  - About has *Check for updates automatically* (on by default) and *Check for updates*.
- **Release channel:** `tools/android/release_channel.py` holds the rules that make every
  published APK install over the beta.
  - The locally signed publisher refuses a version code that is not higher than every
    published release. It also writes an invisible `vidcue-android-update` marker into the notes;
    the app reads only marked releases.
  - The CI Preview publisher refuses to run once a marked release exists, because debug-signed
    Preview APKs cannot update the beta.
  - Release builds require an explicit `YTUI_ANDROID_VERSION_CODE`.
  - A helper derives codes from beta/rc/final names (`0.3.0-beta.1` → 30001), all above the
    published alpha codes.
- **Fix:** the empty-title export name was renamed to `vidcue-media` in one place only. The unit
  test and `OfflineExport` defaults now match. This came from the rename commit and was caught by
  running the Android unit tests.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Android unit tests | `gradlew :app:testDebugUnitTest` (local SDK 35; Maven Central mirror for dependencies) | 119 tests, 0 failures | unit (Robolectric) |
| Update logic | `AppUpdatesTest`: marker parsing, Preview/draft/foreign-URL rejection, same package/key/higher code/minSdk choice, manual vs automatic check, skip, setting default, checksum-verified download, non-APK rejection | pass | unit (loopback server) |
| Instrumentation sources compile | `gradlew :app:compileDebugAndroidTestKotlin` | pass | compile |
| Lint | `gradlew :app:lintDebug` | 0 errors (style warnings only, as elsewhere in the app) | lint |
| Release rules | `python3 -m unittest discover -s tools/android -p 'test_*.py'` | 22 tests pass | unit |

## Not verified and limits
- No emulator or physical device ran the flow. Not yet exercised: Android's confirmation screen,
  the "Install unknown apps" round trip, and the restart into the new version.
- First physical check: install beta.1, publish beta.2, then update from About → Check for
  updates. Confirm the account, settings and library survive.
- The marker is written only by future publishes. The published alphas carry none, so they are
  never offered. The first update anyone receives is beta.1 → beta.2.
- Release builds were not built here: the owner key is private and the NDK was not installed.
  Only debug compile, unit tests and lint ran.
- GitHub's unauthenticated API allows 60 requests per hour per IP address. The 6-hour interval
  keeps the automatic check well below that.

## For the plan and parity tables
- Android parity: "In-app updates: signed builds offer newer owner-signed releases (same package,
  same key, higher code) with Update now / Later / Skip, using Android's installer. Unit verified;
  physical update acceptance open."
- Release process: "From the beta on, only owner-signed releases are published. Version codes
  must be higher than every published release (enforced), and CI Preview releases are refused."
