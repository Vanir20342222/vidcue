# Convert local media to MOV video and ALAC audio (M2)

- **Date:** 2026-11-10
- **PR:** not opened (branch pushed for review)
- **Milestone:** M2
- **Platforms:** shared engine
- **Status:** fixture/unit verified

## What it does

Local conversion and the shared download-option validator now accept two more
outputs: `mov` for video, and `alac` for audio, which writes Apple Lossless
audio into an `.m4a` file. ALAC is lossless, so the audio quality setting does
not apply to it. These are the two targets the pinned, vendored yt-dlp-rs engine
already accepts for native downloads, so the validator now matches what that
engine can write. Before this change a `mov` or `alac` choice was rejected before
any work started.

Details:

- `DownloadOptions::validate` allows `mov` as a video container and `alac` as an
  audio container.
- `audio::encoding` maps `alac` to the FFmpeg `alac` encoder and skips quality
  arguments for it, the same as FLAC and WAV.
- `convert_local` names the output through `output_extension`: `alac` writes
  `.m4a` and `vorbis` keeps writing `.ogg`.

Not included: AIFF and OGG audio (the vendored engine rejects them as audio
targets), AVI and FLV video, and any Android transcoding. Android exports still
only mux or copy (see `docs/android/PARITY.md`).

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Formatting | `cargo fmt --all -- --check` | Pass | - |
| Strict lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass | - |
| Container validation | `cargo test -p vidcue-core download_containers_match_what_the_engine_can_write` | Pass | fixture/unit verified |
| ALAC encoder arguments | `cargo test -p vidcue-services lossless_formats_take_no_quality_arguments` | Pass | fixture/unit verified |
| Output extension names | `cargo test -p vidcue-services conversion_extensions_name_the_container_ffmpeg_writes` | Pass | fixture/unit verified |
| Real FFmpeg conversion | `cargo test -p vidcue-services local_conversion_writes_mov_and_alac_outputs -- --ignored` on a synthetic H.264/AAC MP4 | Pass: MOV holds H.264 and AAC; `.m4a` holds ALAC | fixture verified |
| Workspace suite | `cargo test --workspace --locked` | Pass: 334 passed, 0 failed, 19 ignored (live and tool-dependent tests) | unit verified |

## Not verified and limits
- No UI exposes these formats yet. The desktop app was removed on 2026-11-08,
  so the only callers are backend conversion jobs.
- Online downloads into MOV or ALAC were not run: no live YouTube access in
  this environment. Online MOV output uses the engine's stream copy (`-c copy`),
  so a source whose codecs MOV cannot hold will not produce a valid file. This
  is untested.
- The real-FFmpeg check used one synthetic input. Other source codecs were not
  checked.
- Android has no MP3, FLAC or ALAC transcoding. That needs MediaCodec encoders
  and device verification.

## For the plan and parity tables
- M2, local conversion: MOV video and ALAC audio outputs are accepted by the
  shared validator and encoded with FFmpeg. Fixture verified with real FFmpeg on
  synthetic input (`cargo test -p vidcue-services -- --ignored local_conversion_writes_mov_and_alac_outputs`).
- Android transcoding (MP3, FLAC, ALAC) remains unsupported.
