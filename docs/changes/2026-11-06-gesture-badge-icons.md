# Vector icons in the player gesture badge (M9)

- **Date:** 2026-11-06
- **PR:** #<number>
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does
The on-video badge shown for volume, brightness, double-tap seek and hold-for-speed gestures now
draws white vector icons instead of the ☀, 🔊, ⏪ and ⏩ text characters, which each device drew in
its own emoji style. The volume icon shows muted (0%), one wave (1-49%) or two waves (50%+); the
brightness sun has short rays below 50% and long rays from 50%. The owner picked the solid set
from a preview on 2026-11-06.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Drawable XML parses | `xml.dom.minidom` on each `ytui_gesture_*.xml` | pass | unit |
| Drawable geometry | Path data rendered as SVG in Chromium on the badge colours | icons as previewed | unit |

## Not verified and limits
- No Android SDK in the authoring environment: the app was not compiled locally; PR CI compiles it.
- Not seen on an emulator or physical device.

## For the plan and parity tables
- M9 player gestures: badge icons are vector drawables (`ytui_gesture_*`), not emoji text.
