# Restore Android embedded JavaScript runtime selection (M9)

- **Date:** 2026-11-06
- **Milestone:** M9
- **Platforms:** Android / shared engine
- **Status:** fixture/unit verified

## What it does

The reported video-playback error ends with “no eligible media formats; the
configured JavaScript runtime is unavailable or unsupported”. Android selected
`vidcue-android-js` after the application rename, while the vendored extractor
recognized only `ytui-android-js`. Challenge-bearing streams therefore tried to
resolve a nonexistent executable instead of calling the registered Android
isolated JavaScript evaluator.

Export `yt_dlp_rs::EMBEDDED_JS_RUNTIME` and use it in mobile playback and all
extractor runtime recognition, naming and dispatch paths. Preserve the existing
protocol identifier independently of the application name. No external runtime
installation or backend change is required.

An offline regression registers a synthetic host evaluator and verifies that the
public selector reaches it without an executable, reports `android-isolate`,
returns the validated challenge solution and still rejects mismatched result
keys. This tests routing and validation, not System WebView execution.

## Evidence

Checks run:

- `cargo test -p vidcue-mobile --lib --locked`: 36 passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 461 passed, 31 explicitly ignored, zero failed.
- `CARGO_TARGET_DIR=$PWD/target cargo test --manifest-path vendor/yt-dlp-rs/Cargo.toml --lib --locked extractor::youtube::challenge::`: 12 passed. Optional real-runtime cases return early without their environment variables.
- `tools/test-native-solver.sh`: 2 passed with actual Node, covering saved-player vectors and permission denial.
- `cargo fmt --all -- --check`, direct `rustfmt --check --edition 2024` on the changed vendored runtime, and `git diff --check`: pass.

## Not verified and limits

No Android SDK is configured in this workspace. APK build, emulator/physical
Android evaluator execution and live YouTube playback were not run. An updated
APK containing this source change is needed for the installed app to benefit.
M9 physical/live acceptance remains open.

## For the plan and parity tables

M9 Android native extraction uses the extractor’s shared embedded-runtime selector,
fixing the app-rename mismatch that rejected JavaScript-challenged video streams.
Offline routing, strict response validation and saved Node solver vectors pass;
updated APK and physical/live YouTube playback acceptance remain pending.
