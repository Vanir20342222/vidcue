# Android visualisers, cover fixes and scroll/download polish (M9)

- **Date:** 2026-11-04
- **PR:** see the pull request for this branch
- **Milestone:** M9
- **Platforms:** Android
- **Status:** emulator verified for round two (76/76, 792c887); visualiser and cover-fix builds: see the PR's CI

## What it does

- Visualisers take their colours from the cover: up to three dominant colours, blended across the bars, rings and strings.
- Beats come from spectral flux with an adaptive threshold, so soft piano notes count as beats. Beat strength varies.
- Pulse orbit reacts to each beat: the ring jumps outwards, the cover thumps, strong beats shake the stage slightly. The ring spins faster when the music is louder. Sparks were tried and removed.
- Strings are plucked on each beat and move faster with the music.
- Android no longer offers the full-screen music styles.
- The cover stays centred when paused (it was shifted to the bottom right).
- A cover no longer goes missing after Next: stream metadata only fills in a missing cover, a failing YouTube cover falls back to the standard image, and the last cover of the same track stays visible while a new one loads.
- Search header and feed chrome slide with the scroll; the page under an open music player stops drawing.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Beat and auto-gain maths | Kotlin harness over `SpectrumMath.kt` | Piano 15/15 notes, noise 0 false beats, peak hold/fall correct | unit (harness) |
| Rust providers/engine | `cargo test -p ytui-providers -p ytui-mobile --locked` | 50 + 17 + 73 passed | unit |
| Lint | `cargo fmt --all -- --check`, `cargo clippy -p ytui-providers -p ytui-mobile -D warnings` | Clean | static |
| Emulators, round two build | CI run 41862590768 (792c887) | 76/76 instrumentation tests on 4 emulators | emulator |
| Kotlin compile of this round | CI `apk` job on the PR head | See PR checks | CI |

## Not verified and limits

- The visualiser colours, beat motion and cover fixes have not been run on a phone.
- Emulator frame times are software-rendered and only comparable between CI runs.
- Kotlin unit tests (`ArtworkSwatchesTest`, `SpectrumMathTest`) run in the CI unit-test job, not locally.

## For the plan and parity tables

M9 Android: visualisers use the cover's own colours and beat-driven motion (spectral-flux beats, pulse orbit ring/cover/shake reaction, plucked strings). Full-screen music styles are not offered on Android. Covers stay centred when paused and no longer disappear after Next. Emulator-verified at 792c887 for the earlier round; the visualiser and cover changes await device checks.
