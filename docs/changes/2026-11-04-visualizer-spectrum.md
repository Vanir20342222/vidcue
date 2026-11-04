# Frequency-driven Android visualisers (M9 Android; M5–M8 audio-reactive views)

- **Date:** 2026-11-04
- **PR:** none (committed to the designated working branch)
- **Milestone:** M9 (Android native Compose player views, shared Rust meter); extends the
  M5–M8 Neon VU / Pulse Orbit work
- **Platforms:** shared engine (`crates/ytui-mobile`), Android (Compose). Desktop terminal
  views are unchanged.
- **Status:** unit/fixture verified on the host. Android APK build, Compose rendering,
  on-device instrumented tests and output-latency parity are **not** verified (see below).

## What changed

### Before
- Neon VU drew 24 bars from the top edge, driven by stereo RMS and a fixed sine wobble. The
  bars did not follow frequency content.
- Pulse Orbit drew 12 dots at fixed angles, scaled by the same stereo RMS.
- Neither view smoothed levels. Each frame read the latest raw meter value.

### After
- **Real frequency bands.** The decoder tap (`AudioTap`, PCM16 only, no capture device)
  now also feeds a rolling 1024-sample mono window to a Hann-windowed FFT in
  `crates/ytui-mobile/src/meter.rs`. It returns 24 log-spaced bands from 32 Hz to 16 kHz
  (0–65535 each). Bands get a gentle treble tilt because music's power per bin falls with
  frequency. The stereo RMS/peak word is unchanged and still drives the L/R meters.
- **Smoothing.** `SpectrumSmoother` (`SpectrumMath.kt`) uses frame-rate-independent
  attack/release time constants: bands attack in about 30 ms and release in about 220 ms.
  Peak caps hold for 0.45 s, then fall steadily. The step is clamped to 100 ms, so a stalled
  frame cannot jump the bars. Pause keeps the last shape, because the update loop stops and
  the tap is disabled. Reduced motion never starts the loop, so the bars stay still. A
  zero-step frame changes nothing.
- **Colour.** Band colours are derived from the theme accent by rotating its hue from about
  48° cool to 52° warm across the bands. Every built-in and imported palette keeps its own
  identity. Neutral accents fade towards the foreground colour instead of inventing a hue.
- **Neon VU** (`drawNeonSpectrum`, `drawStereoMeter`): bars rise from a shared floor, with a
  glow, a faint reflection and falling peak caps. A thin L/R stereo meter sits underneath.
- **Pulse Orbit** (`drawPulseOrbit`): one dot per band, fixed at its own angle so each
  frequency keeps its place. Dot size and distance follow the band level. A breathing ring
  follows the bass bands, and a peak highlight rotates on its own clock.
- The per-frame Compose state is one `frame` counter plus the smoother. The smoother is not
  Compose state, so the redraw count is bounded by the animation frame.

## Files
- `crates/ytui-mobile/src/meter.rs`: `Spectrum` analyser, `spectrum_pcm16`, six host tests.
- `crates/ytui-mobile/src/lib.rs`: `with_direct_pcm` helper shared by `meter` and the new
  `spectrum` JNI export.
- `apps/android/app/src/main/java/dev/ytui/android/NativeBridge.java`, `AudioTap.java`:
  the `spectrum` native declaration and the in-place `spectrum` array fed from the tap.
- `apps/android/app/src/main/java/dev/ytui/android/SpectrumMath.kt`: pure smoothing and colour
  maths (no Compose dependency, so it can be unit-tested on the JVM).
- `apps/android/app/src/main/java/dev/ytui/android/AudioVisualizers.kt`: Compose draw helpers.
- `apps/android/app/src/main/java/dev/ytui/android/ComposeHost.kt`: `Visualizer` uses the
  helpers; the Visualizer's public signature and test tags are unchanged.
- Tests: `SpectrumMathTest.kt` (JVM), and `RuntimeTest.decodedToneProducesFrequencyBandsNearItsPitch`
  (instrumented, not run here).

## Evidence

Run on this box, Rust 1.99.1 (the workspace `rust-version`):

| Check | Result |
|---|---|
| `cargo test -p ytui-mobile --locked --lib` | 49 passed, including the 6 new meter tests (silence and unresolvable rates, 1 kHz placement, 90 Hz / 9 kHz ends, malformed and short buffers, newest-window behaviour) |
| `cargo fmt -p ytui-mobile --check` | clean |
| `cargo clippy -p ytui-mobile --locked --lib --tests` | no warnings in `meter.rs` |
| Kotlin: `SpectrumMath.kt` + `SpectrumMathTest.kt` compiled with the Kotlin 2.0.21 compiler bundled in the Gradle install, `-Werror`, then run under JUnit 4.13.2 | 8 of 8 passed |
| `AudioTap.java` + `NativeBridge.java` type-checked with `javac -Xlint:all` against Media3 stubs | clean |
| JNI `spectrum` / `meter` signatures compile-checked against `jni` 0.21.1 in a scratch host crate | compile clean (flagged and removed an unneeded `mut`) |

## Not verified

- **Android build.** The Android SDK/NDK is not installed here. `cargo check --target
  aarch64-linux-android` stops in the `ring` build script (it needs an Android C toolchain),
  so the `android` JNI module itself is covered only by the signature check above.
  Gradle/APK compilation of `AudioVisualizers.kt` and the edited `ComposeHost.kt` also has
  not run. Those Compose calls use APIs that this codebase already uses elsewhere, but a
  compile is still required.
- **On-device.** The instrumented tone test, the existing CatalogUiTest visualiser checks,
  and any Galaxy Tab or phone run have not been executed. Visual quality, frame pacing and
  colour in real themes are unmeasured. No new screenshots were taken.
- **Latency parity.** The tap reads decoder output, before the audio sink's buffer. The
  picture can lead what the speaker plays by the device's output latency. That offset has
  not been measured, and no correction is applied.
- **Calibration.** The dB gain, the tilt and the band edges (`GAIN_DB`, `TILT_DB_PER_OCTAVE`,
  the 32 Hz–16 kHz range) are reasoned from the FFT scaling and the existing -60..0 meter
  scale. They have not been tuned against real music.

## Not changed (follow-ups)
- **Desktop Neon VU / Pulse Orbit** still use mpv `astats` stereo RMS and peak. The docs
  state that they are not frequency bands. A band view there would need a new mpv filter
  chain, and it could not be validated here.
- **`crates/ytui-mobile/src/ui.rs`** keeps its older egui visualiser. The Android app uses the
  Compose views, so this path is not on the user's screen.
