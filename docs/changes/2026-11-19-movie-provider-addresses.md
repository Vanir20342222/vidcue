# M9 Movies provider address repair

Updated Android engine and Cinecat CLI default ZCDN origins to the worker domain
published by https://cinecat.eu/config.js on 2026-11-19. The opt-in public provider
test uses the same current primary origin. Provider ordering and rate-limit
handling remain as implemented.

The installed app screenshot showed three HTTP 429 responses, a rejected network
block video and a sources catalog HTTP 401. Direct requests reproduced HTTP 429
on both the old and current primary origins for TV 105248 season 1 episode 1,
and HTTP 401 on the sources catalog. Updating stale addresses therefore does
not establish restored playback. No successful episode playback or device
acceptance is claimed. This source change has not been installed on the tablet.

Validation completed with 9 mobile Movies tests passing (2 live tests ignored)
and 28 Cinecat provider tests passing (3 opt-in tests ignored). Git diff
whitespace checks pass. Workspace formatting check reports existing formatting
in the concurrent movie catalog changes in lib.rs and movie_catalog.rs.
No APK build or physical-device playback test was performed.
