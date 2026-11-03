# Android watch page and overflow menu polish (M9)

- **Date:** 2026-11-03
- **PR:** <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled locally; see limits)

## What it does
Follow-up to user screenshots of the watch page and the top-bar overflow menu.

- Watch page header: title, then compact views and age ("283K views · 7 months ago"),
  then a channel row with an initial avatar. The same compact format applies to feed cards.
- Action row: unlabeled icons become labelled pills (Queue, Save, Sleep; Lyrics for music),
  with More pinned to the trailing edge. The duplicate fullscreen icon is gone; fullscreen
  stays on the video chrome and swipe-up gesture. Accessible names are unchanged.
- Inline video chrome: below 560dp wide the rewind/forward discs are hidden so prev/play/next
  no longer crowd or clip the 16:9 frame. Double-tap seek still covers rewind/forward;
  fullscreen keeps all five. Time reads `2:33 / 32:46`.
- Up next: when there is no manual queue the source ("Recommended" / "From this playlist")
  sits in the Up next header instead of as a second, misaligned heading. Rows get more space.
- Overflow menu: Browse (grid), Refresh feed (circular arrow) and Sleep timer (moon) get
  their own icons, so no two rows share one. Feed actions are grouped first, About last, with
  dividers, on a rounded surface.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Relative date unit test | `RelativeUploadDateTest` | added, not run here | unit |
| Kotlin compile / instrumentation | Android CI on the PR | pending | CI |
| Device tests follow the new chrome | `MediaRuntimeTest` enters fullscreen through the on-video control; `ContrastUiTest` asserts a compact inline frame hides rewind/forward, then shows them to keep checking their contrast and 48dp targets | pending CI | emulator |

## Not verified and limits
- No Android SDK in the authoring container; the code was not compiled or rendered locally.
- No emulator or physical-device screenshots of the new layout yet.
- `CatalogUiTest` still checks Queue at the leading edge and More at the trailing edge of the
  action row; both hold by construction but were not run.

## For the plan and parity tables
- Android watch page: labelled actions, compact metadata, compact inline transport (M9, UI polish).
