# Tablet library sidebar (M9)

- **Date:** 2026-11-16
- **PR:** pending
- **Milestone:** M9
- **Platforms:** Android
- **Status:** implemented

## What it does

On tablets (window at least 840dp wide), a full-height sidebar attached to the left edge replaces
the floating icon-only navigation pill. Each destination shows its icon and its name. A Library
group opens the playback queue, recently played and the sleep timer, and the foot of the sidebar
holds the account and settings buttons, which leave the toolbar on tablets. Windows narrower than
1000dp get a compact form of the same sidebar: icons with short labels under them, without the
Library group (those panels stay in the ⋯ menu). Phones are unchanged.

The owner chose this design ("B: Library sidebar") on 2026-11-16 from three mockups. The mockup's
search box and now-playing card are left out of the sidebar: the browse page already has its own
search field, and on tablets the player panel on the right already shows what is playing.

Test tags are kept (`navigation-rail`, `tab-N` with the Tab role and destination name,
`settings-button`, `account-button`), so the existing instrumentation tests still address the same
controls. Downloads still catches the flying cover from Save and bounces.

## Evidence
| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Compile app and instrumentation sources | `apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` | passed | implemented |
| Unit tests and lint | `apps/android/gradlew -p apps/android :app:testDebugUnitTest :app:lintDebug` | passed | fixture/unit |

## Not verified and limits

- Not run on an emulator or a physical tablet; no screenshot of the built app was taken here.
  The nightly emulator suite (tablet-size `CatalogUiTest` cases) is the first run on a device.
- The compact sidebar between 840dp and 1000dp has no Library group.

## For the plan and parity tables

- Android tablet navigation: full-height labelled sidebar with Library shortcuts (queue, history,
  sleep timer) and account/settings at its foot; compact labelled rail below 1000dp. Implemented,
  compile/unit/lint verified.
