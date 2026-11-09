# App update checks accept the renamed repository (M9)

- **Date:** 2026-11-09
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Installed beta releases query the release list through the restored repository
(`Vanir20342222/ytui`) and accept APK asset URLs under both the restored and the
briefly renamed (`Vanir20342222/vidcue`) `releases/download/` prefix. Previously a
renamed-repository response was parsed, but every release was then discarded
because its `browser_download_url` no longer matched the pre-rename prefix, so
About → Check for updates reported "up to date" while newer releases existed.

The in-app install path applies the same acceptance rule. Download integrity is
unchanged: the pinned SHA-256, signing certificate, package name and higher
version code are still required before anything is handed to Android's
installer.

## Cause

The repository was renamed after beta.7 (2026-11-06) was published. beta.7's
hard-coded legacy download prefix no longer matched GitHub's canonical asset
URLs, so the first publication after the rename (beta.8) stayed invisible to
the in-app updater. The owner restored the `ytui` name on 2026-11-09 and this
fix ships in beta.9 with the `vidcue` prefix kept as an accepted fallback.
Existing beta.7 installations can update in place again once the name is
restored, and beta.9 also covers devices that installed during the window.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Update-check unit tests | `./gradlew :app:compileDebugKotlin :app:testDebugUnitTest --tests dev.ytui.android.AppUpdatesTest` | 7 tests, 0 failures | fixture/unit verified |
| Legacy-URL acceptance case | new test release with `ytui` prefix asset URL | kept; unrelated host still rejected | fixture/unit verified |
| Live API behavior | `curl` of the old `ytui` releases API with redirect follow | 301 to `vidcue`; asset URLs carry the new name and the beta.8 marker | live verified |

## Not verified and limits

- No real device check yet: the beta.7 → beta.8 in-place update through the
  updater cannot work by definition until this fix ships in beta.9, because the
  fix is client-side. beta.8 must be sideloaded once on beta.7 devices.
- The update marker, download verification and installer flow are unchanged and
  were not re-tested on a device.

## For the plan and parity tables

No plan or parity row changes: update-check URL acceptance only.
