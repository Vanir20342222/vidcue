# M9: Android native bundle publication

Scope: only the local Cinecat directory-publication blocker. The transfer source
at the requested baseline `024e8ec` and the shared checkout's initial HEAD
`06d0479189e720cdda095fd90fa63deae64ce801` has the same blob
`681cd6e80fb0ccd6cd21be33bb5f63ff876c8763`.

Android previously selected the unsupported-platform branch after downloading
into staging. It now uses the kernel `renameat2` syscall with `RENAME_NOREPLACE`.
This avoids bionic's API-30-only `renameat2` symbol while supporting the app's
API-26 baseline. Linux retains its existing libc `renameat2` call. Other platforms
remain unsupported. Unsupported kernels/filesystems return an error; there is
no copy, plain rename, or check-then-rename fallback.

The existing sibling staging directory is still published by one atomic
no-overwrite operation. Existing files, directories (including empty ones), and
dangling symlinks cannot be replaced. A failed publication leaves staging under
its current owner, so the downloader's TempDir cleanup still applies.

## Local validation

- `cargo test -p vidcue-providers --locked`: 140 passed, 0 failed, 4 ignored;
  doc tests: 0 tests, successful. Includes existing HLS/encrypted/ranged bundles,
  progressive downloads, excerpts, caption bundles, cancellation and rollback.
- Three new tests: complete directory move with a Unicode path; late collisions
  with empty/nonempty directories, files and dangling links; simultaneous
  publishers with exactly one winner and intact losing staging.
- `YTUI_ANDROID_RUST_ONLY=1 tools/android/build.sh`: ARM64 release JNI build
  succeeded (`cargo build -p vidcue-mobile --target aarch64-linux-android
  --release --locked -j 2`, with the script's NDK API-26 linker environment).
  The script's subsequent optional x86-64 build was stopped after over eight
  minutes waiting for another build's shared Cargo lock; x86-64 is unqualified.
  To repeat only the completed ABI: `YTUI_ANDROID_ABIS=arm64-v8a
  YTUI_ANDROID_RUST_ONLY=1 tools/android/build.sh`.
- NDK `llvm-readelf --dyn-syms target/android-jni/arm64-v8a/libvidcue_mobile.so`:
  `syscall@LIBC` is imported; `renameat2` is not imported. The installed NDK's
  `stdio.h` declares the libc wrapper `__INTRODUCED_IN(30)`.
- `rustfmt --edition 2024 --check crates/vidcue-providers/src/cinecat/transfer.rs`
  and `git diff --check`: passed.
- No device access, installation, live service or playback qualification.

## Parent's JNI qualification on an explicitly selected disposable app

Build/package this exact patched Rust library; do not test an older APK. Use a
fresh directory under the target app's cache (not the user's movie library).
Run `NativeBridge.request` on a worker thread after normal engine initialization;
if testing without app startup, call `NativeBridge.init(filesRoot, cacheRoot)`
with disposable absolute roots and require an empty error string first.

Serve deterministic fixtures from an instrumentation-owned loopback HTTP server
on the device. For publication-only coverage, `/clip.mp4` can contain known bytes;
`/list.m3u8` can contain the following finite playlist, with `/segment.ts` serving
known bytes. These synthetic bytes qualify storage, not decoding:

```text
#EXTM3U
#EXT-X-TARGETDURATION:2
#EXTINF:2,
segment.ts
#EXT-X-ENDLIST
```

Call JNI with this JSON (substitute the fixture port and fresh absolute output):

```json
{"op":"movie_download","source":{"kind":"hls","url":"http://127.0.0.1:PORT/list.m3u8","subtitles":[{"url":"http://127.0.0.1:PORT/caption","language":"en","display":"English","format":"vtt"}]},"output":"/ABSOLUTE/DISPOSABLE/ROOT/hls"}
```

`/caption` should return `WEBVTT\n\n00:00.000 --> 00:02.000\nHello\n`.
Require the response envelope's `ok` to be true; `data.entrypoint` must exist
inside the output; `data.subtitles` must equal 1. Check `download.json`, the local
playlist/segment bytes and `caption-0.vtt`. Repeat with `source.kind = "file"`,
`source.url = "http://127.0.0.1:PORT/clip.mp4"`, no subtitles, and a different
output. Require exact `video.media` bytes and a successful receipt. Retry each
same output and require `ok:false` with all prior bytes unchanged.

For the late collision test, block the server before returning a segment/body,
create an empty output directory after the fetch begins, then release the body.
Require `ok:false`, the empty directory preserved and no `.vidcue-cinecat-*`
staging left after the request completes. Repeat with a sentinel file, a
nonempty directory and a dangling symlink. With two blocked downloads targeting
one absent output, release both and require exactly one success and a complete
winner bundle; verify that the loser leaves no staging. Keep server waits bounded
and clean up only this test's own roots.

The existing JNI `movie_download` operation exposes complete HLS/file bundles,
including attached captions. It does **not** expose provider `sample_seconds`
or standalone `download_subtitles`. Their provider tests cover the same fixed
publication helper on Linux; qualifying those exact APIs on Android would need
a separate test harness, not invented JNI request fields. Physical/runtime
acceptance remains the parent's work; cross-compilation alone does not close M9.

## Galaxy Tab S9 validation

Three installed-Preview JNI movie saves of the public 11,053,871-byte trailer failed at directory publication on the PR baseline. With the patched arm64 library each save succeeded and matched the independently obtained SHA-256. Elapsed times were 7.211, 12.888 and 10.167 seconds. The first UI candidate was repackaged with the exact rebuilt library and its Preview signer verified. Device evidence is in `target/android-performance-evidence/tablet-024e8ec/final/publication.txt` and `publication-{1,2,3}.log`.

The arm64 Cinecat provider suite also ran on the same Galaxy Tab S9. It passed 31 tests with zero failures, including encrypted ranged HLS, progressive MP4, captions, interrupted downloads, destination collisions and concurrent publication. The suite skipped two opt-in live-provider tests and one FFmpeg/ffprobe decode test. The 113 filtered tests belong to other provider modules. See `target/android-performance-evidence/tablet-024e8ec/final/native-android.txt`.

## Physical device (Galaxy Tab S9, 2026-11-18)

Native movie saves of the public 11,053,871-byte W3C trailer on the SM-X710 (Android 16,
Preview package): baseline 3 of 3 failed at publication; with this change 3 of 3
completed with the expected size and matching SHA-256, median 10.17 s (1.04 MiB/s).
Evidence: `target/android-performance-evidence/tablet-024e8ec/` (local).

## Rebased checks (on e7fe3ae)

`cargo fmt --all -- --check`, `cargo clippy -p vidcue-providers --all-targets --locked -- -D warnings`
and `cargo test -p vidcue-providers --locked` (143 passed, 4 ignored) passed.
