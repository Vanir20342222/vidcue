# Restore native YouTube livestream playback (M9)

The on-air badge work in PR #99 did not change the native extractor's explicit
rejection of live YouTube videos. That guard produced the Android snackbar
`unsupported YouTube behavior: live, upcoming and post-live videos` before a
stream could reach Media3.

The opt-in playback extractor now accepts currently live broadcasts and selects
HLS variants. It excludes direct HTTPS formats that require YouTube's sequence
fetching, discovers the live master even on the direct-player shortcut, and
leaves child-playlist loading, segment loading and refreshes to Media3. VOD
direct-quality comparisons no longer reject live HLS based on unusable HTTPS
offers. Full reacquisition reuses identical live manifest catalogs within that
request, without caching them across reconnects.

Upcoming broadcasts, post-live processing and live downloads remain unsupported.
Archived broadcasts and ordinary VOD retain their existing extraction paths.
No UI, playback-backend settings, external executable fallback or release
identity changes are involved.

## Validation

- `CARGO_TARGET_DIR=target cargo test --manifest-path vendor/yt-dlp-rs/Cargo.toml --offline --locked --test mobile_playback`:
  29 passed. Synthetic TLS proxy fixtures cover initial live video/audio,
  full reacquisition, alternative-client recovery, HLS-only selection despite
  direct-format offers, one master fetch per extraction, no child/segment
  fetches, live metadata, and retained upcoming/post-live/download rejection.
  Existing VOD quality, account-denial, rate-limit and request-budget tests pass.
  The proxy requires permission to bind localhost sockets; tests were rerun
  with networking enabled after the default sandbox denied those binds.
- `cargo test -p vidcue-mobile --lib --offline --locked`: 52 passed, including
  live metadata and playlist URL propagation through Android's stream adapter,
  HLS preparation and exclusion of live streams from cached resolution leases.
- `cargo clippy -p vidcue-mobile --lib --offline --locked -- -D warnings`: pass.
- `cargo fmt --all -- --check` and `git diff --check`: pass.

No Android SDK/device is available here, and the environment's permitted network
destinations do not include YouTube. No APK build, emulator, actual broadcast or
physical-device playback/reconnect acceptance is claimed. The fix must ship in
a rebuilt Android app before it can change an installed version's behavior.
