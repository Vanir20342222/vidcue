# Playback equalizer with presets and custom curves (M5/M7/M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M5 (playback), M7 (settings and first-run setup), M9 (Android)
- **Platforms:** desktop, Android, shared engine (`vidcue-core`, `vidcue-mobile`)
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

### Android
The same settings (`equalizer`, `equalizer_custom` in the shared engine config) drive Android
playback. Settings → Playback and the onboarding Playback step show an equalizer card: a drawn
response curve, preset chips (Off, the built-ins and custom presets) and, for a custom preset, one
1 dB-step slider per band plus Rename and Delete. "New custom preset" / "Customize a copy" copies
the current curve into a new preset (`custom`, `custom-2`, …, up to 16). The onboarding summary
lists the chosen preset. The engine validates every save, as on desktop.

Audio goes through a new Media3 `EqualizerProcessor`, placed before the visualizer tap so
visualizers show what plays. It hands 16-bit PCM to the Rust engine, whose filter bank uses the
same RBJ peaking design as FFmpeg's `equalizer` filter (octave width 1), so a preset sounds the
same on both platforms. A pre-gain equal to the largest boost keeps 16-bit output from clipping;
this makes boosted presets slightly quieter overall than on desktop. The engine applies saved
changes immediately, including to audio already playing; seeks clear filter memory. Off copies
audio through untouched. The curve does not depend on device `audiofx` support.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Filter selection and mpv launch argument | `cargo test -p vidcue-services equalizer saved_copy` (fake mpv records `--af=@vidcue-eq:…`) | passed | fixture |
| Settings round-trip, unsaved custom presets offered, invalid gains rejected | `cargo test -p vidcue forms::tests::equalizer` | passed | unit |
| Setup preset cycling, custom band rows, clamping, summary | `cargo test -p vidcue setup::tests` | 4 passed | unit |
| Filter chain syntax | `ffmpeg -f lavfi -i sine=d=1 -af "equalizer=f=31:t=o:w=1:g=6,…,equalizer=f=16000:t=o:w=1:g=-1.5" -f null -` | accepted | tool |
| Presets, custom parsing, validation, mpv filter text; shared filter bank: −12 dB at 1 kHz measures −12 dB (±0.5), other bands unchanged, state carries across buffers, boost headroom, bands above Nyquist dropped | `cargo test -p vidcue-core equalizer` | 3 passed | unit |
| Engine follows saved settings; off/flat pass audio through untouched; preset catalog | `cargo test -p vidcue-mobile equalizer` | passed | unit |
| JNI wrappers type-check against `jni 0.21.1` | scratch crate with the three new functions | compiled | build |
| Android model: preset list, copy/edit/clamp, rename validation, delete | `:app:testDebugUnitTest --tests dev.ytui.android.EqualizerModelTest` | see PR | unit (Robolectric) |
| Android compile | `:app:compileDebugKotlin :app:compileDebugJavaWithJavac` (SDK 35 installed in the session) | see PR | build |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | see PR | unit |

## Not verified and limits
- mpv is not installed in the development container, so the live `af add/remove @vidcue-eq` path and
  audible output were not exercised; no audio-device or Ghostty acceptance was run.
- yt-dlp fallback playback launches mpv separately; it picks the equalizer up through the IPC sync
  after start rather than from a launch argument.
- Android: no emulator or physical-device run. Audible output, the Compose card's layout, and
  touch input on the sliders were not checked on a device, and no instrumentation test was added.
  The native library was not cross-compiled here (the NDK is not installed); the JNI functions
  were type-checked separately.
- Android processes 16-bit PCM only (the app disables float output). Tunneled or passthrough
  audio, if ever enabled, would bypass the equalizer.
- Band editing for existing custom presets in desktop Settings is text-based; the per-band rows are in setup.

## For the plan and parity tables
- PARITY (desktop): "Audio equalizer: ten bands, 13 presets plus named custom curves, Settings →
  Playback and first-run setup; applied live through mpv's lavfi equalizer. Unit/fixture verified;
  live audio acceptance pending."
- PARITY (Android): "Audio equalizer: same presets and custom curves as desktop, Settings →
  Playback and onboarding; Rust filter bank in a Media3 audio processor. Unit verified; device
  acceptance pending."
