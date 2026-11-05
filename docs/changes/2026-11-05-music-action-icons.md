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
text lines beside an outlined eighth note (the owner chose this over microphone and speech-bubble drafts), save keeps the download tray, and sleep is the crescent moon with a small "z".
Accessible names are unchanged ("Playback queue", "Lyrics", "Save offline", "Sleep timer, N left");
while a sleep timer runs, the moon shrinks inside a round clock ring whose accent arc drains
clockwise from twelve o'clock (eased between the once-a-second ticks), with a dot at its end like a
clock hand. `PlaybackService` now also publishes `sleep_total` (the timer length) next to
`sleep_deadline` so the ring knows what fraction is left; extending the timer restarts the ring
from the new length.
`Glyph.UpNext`/`Glyph.Lyrics` are also used in the overflow menu, mini-pane and player settings.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Countdown ring | Same arc geometry rendered as SVG at 62% left | Ring and hand dot read as a draining clock | visual mock |
| Glyph geometry | Same paths rendered as SVG in Chromium and inspected | Consistent weight and size | visual mock |

## Not verified and limits
- No Android SDK in the session: the Kotlin was not compiled and no APK, emulator or physical
  device check was run. Exact minutes are no longer printed in the tray (still in the
  accessible name, the sleep chip and player settings); the ring shows the proportion left.

## For the plan and parity tables
- Android music player: secondary actions are icon-only with accessible names (needs device check).
