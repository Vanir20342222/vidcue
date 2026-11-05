# Playback equalizer with presets and custom curves (M5/M7)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M5 (playback), M7 (settings and first-run setup)
- **Platforms:** desktop, shared config (`vidcue-core`)
- **Status:** fixture/unit verified

## What it does
Desktop playback has a ten-band audio equalizer (31 Hz to 16 kHz, ±12 dB per band). Settings →
Playback offers **Equalizer** (`off`, thirteen built-in presets such as flat, bass-boost, vocal,
rock, classical and spoken-word, plus any custom preset) and **Custom equalizers**, where named
curves are written as `name=g1,…,g10; other=…`. Presets typed there can be chosen right away, before
saving. First-run setup's Playback step gets an Equalizer row; choosing `custom` adds ten band rows
adjusted with ←/→ in 1 dB steps, and an untouched custom curve is not saved. The setup summary lists
the chosen preset.

mpv receives a labelled `@vidcue-eq` lavfi filter at launch (not for muted previews), and the player
replaces or removes it over IPC when the saved setting changes, so a running video or song picks up
the new curve without restarting. `off` and an all-zero curve add no filter. The default is `off`,
so existing configurations are unchanged.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Presets, custom parsing, validation and filter text | `cargo test -p vidcue-core equalizer` | 2 passed | unit |
| Filter selection and mpv launch argument | `cargo test -p vidcue-services equalizer saved_copy` (fake mpv records `--af=@vidcue-eq:…`) | passed | fixture |
| Settings round-trip, unsaved custom presets offered, invalid gains rejected | `cargo test -p vidcue forms::tests::equalizer` | passed | unit |
| Setup preset cycling, custom band rows, clamping, summary | `cargo test -p vidcue setup::tests` | 4 passed | unit |
| Filter chain syntax | `ffmpeg -f lavfi -i sine=d=1 -af "equalizer=f=31:t=o:w=1:g=6,…,equalizer=f=16000:t=o:w=1:g=-1.5" -f null -` | accepted | tool |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | see PR | unit |

## Not verified and limits
- mpv is not installed in the development container, so the live `af add/remove @vidcue-eq` path and
  audible output were not exercised; no audio-device or Ghostty acceptance was run.
- yt-dlp fallback playback launches mpv separately; it picks the equalizer up through the IPC sync
  after start rather than from a launch argument.
- Android does not have the equalizer yet. It would use ExoPlayer/`android.media.audiofx` and a
  Compose settings screen; the shared config fields (`equalizer`, `equalizer_custom`) are ready for it.
- Band editing for existing custom presets in Settings is text-based; the per-band rows are in setup.

## For the plan and parity tables
- PARITY (desktop): "Audio equalizer: ten bands, 13 presets plus named custom curves, Settings →
  Playback and first-run setup; applied live through mpv's lavfi equalizer. Unit/fixture verified;
  live audio acceptance pending."
- PARITY (Android): "Audio equalizer: not implemented (desktop only)."
