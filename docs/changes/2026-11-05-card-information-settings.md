# M9 — Android card information settings

- **Date:** 2026-11-05
- **Milestone:** M9
- **Platforms:** Android and shared engine
- **Status:** fixture/unit verified

## What it does

The owner reported cards showing only the title and creator with every “What cards
show” switch enabled. Modern YouTube `lockupViewModel` parsing kept the creator
but discarded the second metadata row and thumbnail duration badge. Android also
never rendered `show_description`, restricted statistics to the video tab, omitted
section labels in wide search and shelves, and squeezed creator/statistics into
one ellipsized line.

Modern video lockups now preserve the provider's localized views and publication
labels plus colon-delimited duration badges. Playlist lockups do not interpret
collection statistics or badges as video details. Missing fields stay missing;
the parser does not invent counts or dates or add network requests.

Feed, shelf and wide search cards share `CardInformation` for views, dates, likes,
description and section. Creator text has its own line in feed/shelf cards, leaving
two lines for statistics. Existing channel and duration switches still apply at
their call sites. The settings group explains that individual items may omit
details such as likes or descriptions. No per-card watch-page requests were added;
those details appear only when already present in the media JSON.

## Evidence

- `cargo test -p vidcue-providers --locked --offline browse::`: 48 passed.
  Includes a new synthetic modern-card regression covering localized metadata,
  duration, serialized mobile fields, playlist exclusions and missing data.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p vidcue-providers --all-targets --locked --offline -- -D warnings`: passed.
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: passed
  after applying the fix to the latest `main` for the pull request.
- `cargo test --workspace --locked --offline`: 451 passed, 0 failed, 31 existing
  opt-in ignores after applying the fix to the latest `main`.
- `git diff --check`: passed.
- Added `CardInformationUiTest` for independently toggling each optional detail,
  live recomposition, and null/missing values. Not executed here.
- Attempted `./gradlew :app:testDebugUnitTest :app:compileDebugAndroidTestKotlin
  --offline`; the uncached wrapper attempted to download Gradle 8.14.3 and failed
  with `UnknownHostException: services.gradle.org`. No Android SDK was found in
  the standard local locations. Android compilation, emulator/UI tests, physical
  device verification and live YouTube response verification remain unqualified.

This is a source fix, not a built or published Android release.

## For the plan and parity tables

M9: card information settings apply consistently to feed, shelf and wide search
cards. Modern video lockups retain views, publication labels and duration; optional
descriptions and section labels render when enabled and available. Synthetic
provider regressions pass. Native UI, physical-device and live-service acceptance
remain open.
