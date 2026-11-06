# Icon-only navigation dock with redrawn destination icons (M9)

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented (not compiled or device-checked in this session)

## What it does
The bottom navigation dock and the tablet rail drop their text labels and show only icons
(26dp), matching the icon-only player tray. The page title already names the current
destination. Home, Music and Radio were redrawn on the same grid and 1.8 stroke as the tray
icons: a house with rounded footings and a door, two beamed notes with ring heads (like the lyrics
note), and a rounded radio with antenna, dial and two lines. The selected tab shows the filled
version with its details cut out, plus the existing sliding highlight. Downloads keeps its icon
and the bounce when a save lands. Each tab keeps its name as its content description and the
Tab role.

The dock is now always one 64dp row (the two-row large-font layout existed only to fit the words);
the rail is 72dp wide with 56–68dp items.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Icons | Same paths rendered as SVG in Chromium, each tab selected in turn | Consistent weight, clear filled state | visual mock |

## Test change
`CatalogUiTest.assertNavigationLabelsFit` checked that the label words fit on one line at 2x
font scale. With the words removed at the owner's request, it became `assertNavigationTabsNamed`:
each tab is still displayed, at least 48x48dp, has the Tab role and carries its destination name
as content description. Not run here (no emulator).

## Not verified and limits
- No Android SDK in the session: not compiled, linted, or run on an emulator or device.
- Filled icons cut details out in the theme background colour, which is close to but not exactly
  the highlight colour (same approach as the existing filled glyphs).

## For the plan and parity tables
- Android navigation: icon-only dock and rail with accessible tab names (needs device check).
