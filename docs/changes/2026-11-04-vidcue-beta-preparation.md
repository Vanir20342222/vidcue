# Vidcue rename integration and Android beta preparation (M9)

- **Date:** 2026-11-04
- **PR:** #82
- **Milestone:** M9
- **Platforms:** Android, shared engine, desktop compatibility, CI-release
- **Status:** fixture/unit verified; publication blocked on private signing

## What it does

Finishes the existing Vidcue rename/update PR against main. The two Rust conflicts
were mechanical: retain main's playable nested-collection expansion and MediaKind
import, with the renamed crates. Main's lyric, visualizer, playlist and scrolling
changes are preserved. Android defaults become `0.3.0-beta.1` / `30001`, using the
existing stable package and certificate. Desktop versioning is unchanged.

The signing helper retains the original `ytui-stable.jks` filename, so existing
private recovery archives remain usable. Android update checks report malformed
responses as failures, cancelled downloads remove their partial files, activity
recreation preserves active downloads, and archive verification requires the
advertised signing certificate. Alpha-to-beta release notes describe the required
first manual installation accurately.
The Preview publisher also rejects beta/rc/final version names before any beta is
published, preventing a debug-signed first beta.

The owner's request to finish, land and release this existing PR authorizes its
combined rename, update, version and release-tool changes in place of the default
single-topic and release restrictions in AGENT_PR_RULES. The owner then selected
Android only and confirmed that private signing is currently unavailable.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Rust format | `cargo fmt --all -- --check` | pass | static |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass | static |
| Rust workspace | `CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace --locked` | 434 pass, 0 failures, 31 existing opt-in ignores | unit/fixture |
| Android release helpers | `python3 -m unittest discover -s tools/android -p 'test_*.py'` | 23 pass | unit/fixture |
| Android app and instrumentation sources | `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | pass | compile |
| Android JVM tests | `:app:testDebugUnitTest` | 133 pass, 0 failures/errors/skips | unit/fixture |
| Android lint | `:app:lintDebug` | pass | static |
| CI filters and hygiene checker | `tools/ci/test-should-run.sh`; `python3 tools/ci/test_check_pr.py` | pass; 15 hygiene tests pass | unit |

## Not verified and limits

No owner-signed beta APK or public beta release exists yet. The private signing
setup is unavailable; no signing identity was generated or replaced. Physical
update, Android installer confirmation, live accounts/media and OEM behavior
remain unqualified. [Beta publication handoff](../android/BETA1_RELEASE.md).

Local test infrastructure needed a full JDK/SDK, Java proxy settings and Robolectric's
Maven Central endpoint. The initial Rust test link exhausted workspace disk; generated
builds were cleaned and tests restarted with debug symbols and incremental builds
disabled. No test assertion, timeout or skip policy changed.

## For the plan and parity tables

- M9: Vidcue rename and update support integrated with main; Android beta.1 metadata
  prepared (0.3.0-beta.1/code 30001). Publishing awaits the existing private signing
  setup. This does not mark M9 or physical/live acceptance complete.
