# M9 Aphrodite session handshake repair

The owner reported that Aphrodite plays Cyberpunk: Edgerunners on cinecat.eu
while Android rejects the provider as a network-block error video. The earlier
provider-domain update did not repair this failure.

## Cause and change

Cinecat's current public player bootstraps an anonymous v2 session with
POST /content/index before requesting /content/movie/{id} or
/content/tv/{id}/{season}/{episode}. Vidcue sent an unsigned content request.
For TV 105248 S1E1 that request returned a descriptor whose media was the
known error card. Replaying the current public player request sequence returned
a different playlist.

The Rust adapter now implements the bootstrap HMAC, authenticated AES-GCM
session-envelope decoding and per-request HMAC with a fresh random nonce.
It uses the anonymous public client bootstrap material distributed in the site
player. User credentials are unnecessary. Session keys, signed request headers
and stream URLs stay out of reports and repository fixtures. Session failure
fails explicitly, and the existing error-video fingerprint checks remain active.
A fresh session is established for each resolution, with cancellation, timeouts
and response-size limits. The new direct ring dependency was already present
transitively in Cargo.lock.

The protocol was observed from the public assets served by cinecat.eu on
2026-11-19, including the provider bundle and its bootstrap behavior. The Rust
implementation uses independently written protocol code.

## Evidence

- Disabling the handshake made the TV contract regression fail with a missing
  fixture session header. Restoring the handshake passed.
- Loopback fixtures validate the bootstrap signature, encrypt a synthetic
  session, validate the content-request signature and require session headers
  to stay out of the returned playback descriptor.
- Invalid/truncated/unauthenticated session envelopes and HTTP 401/429 bootstrap
  failures fail without requesting unsigned content.
- Live native resolution for TV 105248 S1E1 with order aphrodite passed the
  three-segment probe in 4.657 seconds.
- A native qualification excerpt downloaded 2,205,728 bytes representing
  10.417 seconds. FFmpeg decoded its video and audio successfully. Inspected
  frames show the Netflix opening and anime introduction. This establishes a
  short media sample, with full-episode playback and tablet acceptance pending.
- Provider tests pass and mobile Movies tests pass. Final counts and Android
  cross-build outcome are recorded below.

No signed URLs, session credentials, downloaded media or public JavaScript
bundles were added to Git. No app was installed on the user's tablet.

Final focused validation passed with 30 Cinecat tests (3 opt-in ignores), 9 mobile
Movies tests (2 opt-in ignores), provider library Clippy with warnings denied,
targeted rustfmt and git diff whitespace checks. The Android ARM64 release Rust
engine compiled and was stripped successfully through tools/android/build.sh
with YTUI_ANDROID_RUST_ONLY=1. This did not build an APK or run a device test.
