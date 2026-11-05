# Lyric fill follows real word timings, not the gap to the next line (M9)

- **Date:** 2026-11-05
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** shared provider crate, Android
- **Status:** fixture/unit verified (Rust); Android not compiled in this environment

## What it does
The karaoke fill on the playing lyric line used to stretch from that line's timestamp to the next
line's, so an instrumental break after a line was counted as singing time and the fill crawled far
behind the voice.

- The LRC parser now reads enhanced LRC word tags (`[mm:ss.xx]<mm:ss.xx>word <mm:ss.xx>word…`) into
  per-word start times (`Line.words`) and a closing time (`Line.end_ms`, from a trailing empty tag).
  The tags no longer show up as text. Global `[offset:]` applies to word times too. Both fields are
  omitted from JSON when absent, so stored data and older readers are unchanged.
- Android fills a line word by word from those times: each word fills from its own start to the next
  word's start; the last word ends at `end_ms`, otherwise after 600 ms, never after the next line.
- Lines with only a start time (plain LRC, YouTube Music) light up whole, as YouTube Music does,
  instead of a fill whose speed would be guessed.
- Copying a found match into the editor keeps its word tags.

Probe (2026-11-05, anonymous, public songs): YouTube Music's ANDROID_MUSIC browse for five songs
returned `timedLyricsData` rows without any `cueRange` timing (client versions 7.27.52 to 8.44.54),
and LRCLIB's 20 synced entries for one song had no word tags. Word-level timing therefore appears
only in imported or saved enhanced LRC files and the rare LRCLIB entries that carry it.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Enhanced LRC parse, offsets, stray `<`, plain lines unchanged | `cargo test -p vidcue-providers --locked lyrics` | 11 passed | unit |
| Workspace | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` | see PR | unit |
| Android sweep and LRC rebuild | `LyricSweepTest`, `LyricLrcTest` | Not run: no Android SDK in the agent container | unit (pending CI) |

## Not verified and limits
- Android code not compiled here; not checked on a device.
- Songs whose lyrics only time lines (most YouTube Music and LRCLIB results) get a whole-line
  highlight, not a word fill. A word fill needs enhanced LRC.

## For the plan and parity tables
- Lyrics: enhanced LRC word timings are parsed; Android fills only word-timed lines and highlights
  line-timed ones whole.
