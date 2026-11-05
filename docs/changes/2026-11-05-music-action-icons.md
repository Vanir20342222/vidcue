# Icon-only music player action tray (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or device-checked in this session)

## What it does
The music player's secondary action tray (queue, lyrics, save, sleep, more) drops its text
captions and shows icons only, at a uniform 24dp. The icons were redrawn on the same 24-unit
grid with the same 1.8 stroke so they read on their own: the playback queue uses a new
`Glyph.UpNext` (list with a play marker, kept separate from the add-to-queue plus), lyrics is a
microphone, save keeps the download tray, and sleep is the crescent moon with a small "z".
Accessible names are unchanged ("Playback queue", "Lyrics", "Save offline", "Sleep timer, N left");
an active sleep timer is shown by accent tint and filled glyph instead of the caption.
`Glyph.UpNext`/`Glyph.Lyrics` are also used in the overflow menu, mini-pane and player settings.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Glyph geometry | Same paths rendered as SVG in Chromium and inspected | Consistent weight and size | visual mock |

## Not verified and limits
- No Android SDK in the session: the Kotlin was not compiled and no APK, emulator or physical
  device check was run. The remaining sleep time is no longer visible in the tray (still in
  the accessible name, the sleep chip and player settings).

## For the plan and parity tables
- Android music player: secondary actions are icon-only with accessible names (needs device check).
