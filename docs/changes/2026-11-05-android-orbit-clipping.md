# Keep the outer Pulse orbit elements inside the stage (M9)

- **Date:** 2026-11-05
- **Milestone:** M9 (Android Compose player)
- **Status:** host unit verified; device rendering unverified

## Change

Pulse orbit previously used half the stage's smaller dimension as its outer
radius. Held peak dots extend beyond bar endpoints, ripples travel beyond that
radius, and beat shake moves their centre. The clipped player stage consequently
cut off the outermost elements, as shown in the supplied screenshots.

The renderer now uses a shared, plain Kotlin geometry calculation. It reserves
space for maximum beat shake, ripple strokes, glow and antialiasing, then limits
bar reach to include the full held peak dot. Normal stages retain the existing
cover gap; very small stages reduce the inner radius to keep bar lengths valid.
The shake allowance stays fixed so the bounds do not pulse between beats.

## Evidence

| Check | Result |
| --- | --- |
| Compile `SpectrumMath.kt` and `SpectrumMathTest.kt` with Kotlin 2.1.21, `-Werror`, JVM target 17 | Passed |
| JUnit 4.13.2 `SpectrumMathTest` | 15 passed, including two new orbit regressions |
| Bounds regression | Covers 64–800 dp stages, densities 0.75–4, kick/energy 0–1, band levels 0–1, full held peaks, glow, inner ring and ripple trajectories |
| Regression sensitivity | A temporary copy using the previous renderer's geometry fails the new outer-extent test on a held peak; repository sources remain fixed |
| `git diff --check` | Passed |

The host has no Android SDK or emulator. Compose compilation, an APK build and
rendered appearance on a device were not verified. These tests check geometric
bounds, not Android pixels; physical-device acceptance remains open.
