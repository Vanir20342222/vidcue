# Liquid Android visualisers (M9 Android; supersedes the drawing in 2026-11-04-visualizer-spectrum)

- **Date:** 2026-11-04
- **PR:** none (committed to the designated working branch)
- **Milestone:** M9 (Android Compose player views); extends the M5–M8 audio-reactive views
- **Platforms:** Android (Compose). The shared engine and the desktop terminal views are unchanged.
- **Status:** pure-maths fixture verified on the host. Compose compilation, APK build, on-device
  rendering and output-latency parity are **not** verified.

## What changed

The 24-bar Neon VU and the 24-dot Pulse Orbit are replaced by a simpler liquid design that
sits with the cover art and uses only the theme's accent colour.

- **Neon VU → liquid tank.** The cover stays on top, and the tank fills the container below it.
  Its surface is four overlapping waves, one per band group (bass, low-mid, high-mid, treble).
  Bass sets the long swells, and treble sets the fine ripples. Silence leaves the surface flat.
  A second, deeper layer trails the front one in phase.
- **Pulse Orbit → liquid ring.** A soft blob wobbles around the 32% circular cover. Four lobe
  harmonics, one per band group, shape the outline. Two layers at different phases give depth.
- **Colour.** Tones come from the theme accent, with a lighter tint towards the foreground
  colour. The earlier hue rotation is gone, so the views match each palette instead of adding
  neon colours.
- **Removed:** the 24 bars, the reflections and glow, the peak caps, the stereo L/R meter, and
  the rotating peak highlight. The views now read four band groups instead of 24 bands.
- **Motion.** Group energies rise in about 60 ms and fall in about 320 ms. The surface and ring
  drift on their own clock, and pause, reduced motion and the lifecycle gates behave as before.
  The clock is read only at draw time, so the animation does not recompose the screen.

## Files
- `apps/android/app/src/main/java/dev/ytui/android/SpectrumMath.kt`: `LiquidLevels`,
  `liquidGroupAverage`, `surfaceOffset` and `orbitOffset`. Plain JVM, no Compose.
- `apps/android/app/src/main/java/dev/ytui/android/AudioVisualizers.kt`: `LiquidTank` and
  `LiquidOrb` composables, and their draw helpers.
- `apps/android/app/src/main/java/dev/ytui/android/ComposeHost.kt`: `Visualizer` now uses the
  liquid composables. The VU cover moved to the top of a `Box`, and the orbit keeps its
  centred cover.
- `apps/android/app/src/test/java/dev/ytui/android/SpectrumMathTest.kt`: 7 tests.

Not changed: `AudioTap` and the Rust meter still produce the stereo word, which the views
no longer read. Removing it is a possible follow-up, but it has its own instrumented tests.

## Evidence

| Check | Result |
|---|---|
| Pure Kotlin (`SpectrumMath.kt`, `SpectrumMathTest.kt`) compiled with the Gradle-bundled Kotlin 2.0.21 compiler, `-Werror` | compiles |
| Same files under JUnit 4.13.2 | 7 of 7 passed: group averages, flat silence, bounded surface and ring at full swell, smooth flow, attack/release, frozen zero steps, overall mean |
| Rust engine | unchanged in this step (the previous 49 tests still apply) |

Previews are in the chat as simulations of the drawing maths, not device captures.

## Not verified
- **Compose compilation.** `AudioVisualizers.kt` and the edited `Visualizer` are not compiled
  here. They use the same Compose APIs as the rest of the codebase: `Canvas`, `drawPath`,
  `Brush.verticalGradient`, `Path`, `Box` and `Modifier.align`. Still, a Gradle build is needed.
- **On-device look and motion.** Nothing has been run on a phone or tablet. The wave amplitude,
  the wobble of the ring and the tonal contrast in each theme all need checking by eye.
- **Latency.** The visual still leads the audio sink by the device's output latency. That
  offset is unmeasured.
- **Calibration.** The amplitude constants (tank `0.05 + 0.10 × overall`, ring amplitude 0.3)
  are reasoned, not tuned on real music.
