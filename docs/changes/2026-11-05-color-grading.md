# Colour grading and auto grade for video playback (M5, M9)

- **Date:** 2026-11-05
- **PR:** #<number>
- **Milestone:** M5 (desktop mpv playback); M9 desktop first-run setup
- **Platforms:** desktop / Android / shared engine (config)
- **Status:** desktop fixture/unit verified; Android implemented (not compiled here)

## What it does
Settings → Playback gains **Color grading** (`off`, `warm`, `cool`, `vivid`, `cinematic`) and
**Auto color grade** (on/off). Both are off by default and both are offered on the Playback step of
first-run setup and listed on its summary. Grading applies to normal video playback only; audio-only
playback and muted `v` previews are untouched, and with both settings off vidcue sends nothing to mpv.

Looks use mpv's video equalizer (`brightness`, `contrast`, `saturation`, `gamma`, applied by the VO).
Warmth uses a labelled FFmpeg `colorbalance` filter (`@vidcue-grade`), added only when non-zero.
Auto grade measures the ungraded picture: three seconds after playback starts it prepends a labelled
`signalstats` probe (`@vidcue-grade-probe`) to the video filter chain, reads up to six frames of
statistics from `vf-metadata/vidcue-grade-probe` within 2.5 s, removes the probe and derives a slight
correction (gamma towards balanced exposure, contrast for flat pictures, saturation towards a
moderate level, a lift-black trim and partial neutralisation of a colour cast). Each term is capped
to a few points on mpv's −100..100 scale; preset plus auto is capped at ±30. It looks again every
30 s and blends new results in by half; a window without usable frames retries after 60 s. Pausing
closes an open probe. Near-black frames are ignored. Bit depth comes from mpv's
`video-params/pixelformat`, because FFmpeg 7's `YBITDEPTH` reports 8 for 10-bit frames.

Settings changes apply to the playing video on the next poll. The new keys are `color_grade` and
`auto_color_grade` in `config.toml`; existing configs load with both off.

**Android** (requested 2026-11-05: "Added to Android app"). Settings › Playback gains a **Color
grading** choice and an **Auto color grade** switch; onboarding's Playback page gains both
(a tap-to-cycle row and a switch) and the summary lists them. They are mobile settings
(`color_grade`, `auto_color_grade` in `MobileSettings`), off by default. `ColorGrade.kt` attaches to
the inline and fullscreen `PlayerView`s and sets a `ColorMatrixColorFilter` on the TextureView's
layer paint (GPU; ExoPlayer and decoding are untouched; with both settings off the view is never
changed). Auto grade takes 48×27 `TextureView.getBitmap` snapshots, which return the ungraded frame,
with the desktop timing (3 s first look, six samples in 2.5 s, every 30 s, blended by half, reset per
media item). `ColorGradeMath.kt` carries the same looks and auto rules as the desktop module. A colour
matrix is linear, so gamma uses its tangent at mid-grey: midtones match mpv's gamma, black and
white move by a point or two.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Grade state machine, presets, stats parsing, bit depth | `cargo test -p vidcue-services --locked color_grade` (8 tests) | pass | unit |
| Settings form round-trip and validation | `cargo test -p vidcue --locked color_grading_settings` | pass | unit |
| Setup rows, defaults and summary | `setup::tests::rows_change_the_draft_only_and_theme_changes_are_reported` | pass | unit |
| `signalstats` key names and `colorbalance` syntax | `ffmpeg -f lavfi -i testsrc2 -vf "signalstats,colorbalance=rm=0.040:bm=-0.040:rh=0.020:bh=-0.020,metadata=print" -f null -` (8-bit and `format=yuv420p10le`), FFmpeg in the CI container | keys match; filter parses; `YBITDEPTH=8` on 10-bit input | tool check |
| Android grade math | `ColorGradeMathTest` (JVM unit test) written | **not run**: no Android SDK in the container, and running a downloaded standalone Kotlin compiler was not permitted | none |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | pass | unit |

## Not verified and limits
- Not run against a real mpv window: mpv is not installed in the build container. The `vf pre` /
  `vf remove` / `vf add` IPC commands, `vf-metadata` reading and the visual result on Omarchy +
  Ghostty are unverified. Adding or removing the probe reinitialises mpv's filter chain, which can
  cause a brief hitch about every 30 s while auto grade is on.
- The warmth filter and the probe run on the CPU. With hardware decoding enabled in a user's mpv
  setup they would force copy-back; vidcue starts mpv with `--no-config`, so decoding is software.
- Auto grade assumes limited-range video (16–235), which covers almost all streamed video.
- Android: not compiled. Run `apps/android/gradlew -p apps/android :app:compileDebugKotlin
  :app:testDebugUnitTest --tests dev.ytui.android.ColorGradeMathTest`. Not checked on an emulator or
  device. Whether a layer-paint colour filter on a TextureView renders correctly on the Galaxy Tab S9,
  including with the fullscreen options blur, is unverified. Android uses its own mobile settings,
  so they do not sync with the desktop `config.toml` keys.

## For the plan and parity tables
- PARITY (desktop playback): "Colour grading looks (warm/cool/vivid/cinematic) and opt-in auto grade
  for video playback; both off by default; in Settings → Playback and first-run setup. Fixture/unit
  verified; real mpv window not yet checked." Android: "Implemented in Settings › Playback and
  onboarding; not yet compiled, emulator or device verified."
- VALIDATION (open gate): "Play a dark and a bright video with Auto color grade on in mpv on the
  target PC; confirm the probe is added and removed, the correction is slight and playback does not
  stutter."
