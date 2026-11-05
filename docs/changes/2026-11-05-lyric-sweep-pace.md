# Lyric fill follows the singing, not the gap to the next line (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (unit tests written; not compiled in this environment)

## What it does
The karaoke fill on the playing lyric line used to stretch from that line's timestamp to the next
line's. Line-synced lyrics (LRC, YouTube Music) only time line starts, so an instrumental break
after a line was counted as singing time and the fill crawled far behind the voice. The fill now
runs over an estimated singing time for the line's text (300 ms per syllable, at least 700 ms),
and only uses the full gap when the next line starts sooner than that estimate. Syllables are vowel
groups in Latin, Cyrillic and Greek text, and one per character in scripts without vowel letters
(CJK, kana, Hangul).

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| New `LyricSweepTest` cases (gap ignored, short gap still finishes, range bounds, syllable counts) | `./gradlew :app:testDebugUnitTest --tests dev.ytui.android.LyricSweepTest` | Not run: no Android SDK in the agent container | unit (pending CI) |

## Not verified and limits
- Not compiled or run here; CI's Android job is the first build.
- Not checked on a device against real songs. The pace is an estimate: held notes and very fast
  verses can still finish early or late. Word-level timings (enhanced LRC) are not parsed yet.

## For the plan and parity tables
- Android lyrics: the karaoke fill follows an estimated syllable pace capped by the next line's
  start, so instrumental breaks no longer slow it (unit-level evidence only).
