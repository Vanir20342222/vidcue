# Android dark band under the player actions (M9)

- **Date:** 2026-11-04
- **PR:** <!-- none opened; branch ccr-6b4775a4-b3d27v -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented by reading the code. Not compiled, not rendered and not run on a device:
  the sandbox has no Android SDK, so `apps/android` cannot be built here.

## What it does

A screenshot of the Now playing panel (Queue, Lyrics, Save, Sleep, More) showed a darker strip
below the action row, above the gesture bar, with a hard top edge.

- **Cause (best reading, not reproduced).** The app paints its background under the navigation
  bar, but `YtuiApp` set the navigation bar to transparent. On Android the system then draws its
  own colour there (a contrast scrim or the theme colour), which does not match the app's
  background. The panel's ambient glow ends at the same `background` colour as the root, so the
  edge comes from the system layer, not from Compose.
- **Fix.** `ComposeHost.kt` now sets the navigation bar to the app's `background` colour (opaque)
  instead of transparent. The root, panels and the bar then show one colour, so the system draws
  nothing different there. Status bar, insets and panel layout are unchanged.

## Checks

- Read the edited block in `ComposeHost.kt`; the `toArgb` import is added and the `background`
  value is in scope within `YtuiApp`.
- Not run: Gradle/Android build, `NativeUiTest`/`CatalogUiTest`, and any emulator or device.

## Open gate

Confirm on the Galaxy Tab S9 and a phone in both light and dark themes, in gesture and 3-button
navigation, that no band remains under the Now playing actions and the sheets still look right.
If a band remains, the next thing to check is the nav-bar colour on Samsung One UI.
