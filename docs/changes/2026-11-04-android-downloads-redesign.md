# M9: readable Downloads page with full transfer details

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified (pure helpers only; the Compose screen was not compiled)

## What it does

The Transfers list was one thin row per download: thumbnail, title, a single
`Downloading · 1% · 8…` line that the Pause and Close buttons cut off, and a
4 dp bar. The owner's screenshot showed the speed and time left unreadable.

Each transfer is now a card. The title (up to three lines), channel and a
coloured status line sit beside the thumbnail. A full-width 6 dp bar follows,
then the numbers on their own lines: percent, "12 MB of 1.2 GB", and while
running the speed and time left ("Measuring speed…" until a second snapshot
gives an honest number; the speed is dropped after 5 s without new bytes).
Pause, Resume, Retry, Restart and Remove are labelled pills that keep the
earlier spoken names. A summary line ("2 downloading · 1 paused") sits above
the list next to "Pause all".

Offline rows gain a details line: "Video · 1080p · 1.2 GB" or "Audio · 54 MB".

`OfflineStore.snapshot()` now adds `quality` to each row and, for a video saved
with a separate audio track, reports the bytes and total of both parts and
derives the percentage from them. Before, bytes and total covered only the video
part while the percentage averaged the two parts, so "40%" could sit beside
"95% of the bytes". Playlist preparation and export rows have no byte counts:
they show their status text with an indeterminate bar while preparing/exporting,
and the word "failed" inside "Preparing 3 / 12 · 1 failed" no longer reads as a
failure.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Size, time-left, percent, details and summary wording | Compiled `TransferFormat.kt` + `TransferFormatTest.kt` with the Kotlin 2.0.21 compiler bundled in Gradle, ran JUnit 4.13.2 directly | 6 tests passed | fixture/unit |
| Java syntax of the `OfflineStore` change | `javac` on the file: only unresolved Media3 symbols, no parse errors | parses | none (not a build) |
| Card states render and their actions fire | `TransferCardUiTest` (androidTest, synthetic rows) | written, **not run** | none |

## Not verified and limits

- The workspace has no Android SDK, and running Google's `sdkmanager` to install
  one was blocked in this session, so `:app:compileDebugKotlin`, the unit-test task, the androidTest APK and every
  emulator or device check were **not run**. `TransferCards.kt`, the edits to
  `OfflineLibrary.kt`/`DesignSystem.kt`, `OfflineStore.java` and
  `TransferCardUiTest.kt` are unbuilt; CI is the first compile.
- No screenshot of the new layout exists. Layout at 2x font scale, on the tablet
  and in the Nothing/imported themes is unchecked.
- Speed and time left come from byte deltas between snapshots (about one per
  second while the library is visible); they were not measured against a real
  download.
- The bytes/percentage change for video+audio saves is untested against real
  Media3 downloads. It leaves the "Waiting for audio" and "Audio failed" states
  as they were.

## For the plan and parity tables

M9 Android: Downloads shows each transfer as a card with status, percent,
"downloaded of total", speed and time left, labelled actions and a summary line,
and Offline rows show kind, quality and size. Six offline JVM tests cover the
wording; APK, emulator and device verification remain pending.
