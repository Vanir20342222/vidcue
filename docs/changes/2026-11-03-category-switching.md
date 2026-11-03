# Immediate cached category switching (M9)

- **Date:** 2026-11-03
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Category taps publish prepared cards synchronously and skip entrance animations.
First pages remain in memory for up to 30 minutes, within 64 entries and an 8 MiB
serialized-text estimate; after one minute they refresh while existing cards stay
visible. Re-tapping a chip reuses its page; pull/menu refresh and retry retain the
explicit refresh path, and pagination and saved items bypass the first-page cache.

Preparation starts without the old 300 ms delay and covers up to 24 server
categories plus Home/account sources, with at most two background fetches across
replaced batches. Connected-account playlist, liked, subscription and watch-later
sources receive early priority. Browsing with a playing mini-player permits
preparation; resolution, the focused player, preload/mobile-data preferences and
service cooldowns still constrain it. Account/index/proxy changes clear private
pages, and foreground taps share pending requests without accepting stale replies.

## Evidence

The branch starts from main `913c2d4`, preserves its newer motion/playback work,
and adds no changes to the shared motion helper. Android checks use SDK 35,
Gradle 8.14.3 and a full JDK 21.

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| JVM regressions | `:app:testDebugUnitTest` | 69 passed, zero failures/skips | fixture/unit |
| App and instrumentation sources | `:app:compileDebugKotlin :app:compileDebugAndroidTestKotlin` (main Java compilation included through test task dependencies) | Passed; instrumentation compiled, not executed | compile |
| Lint | `:app:lintDebug` | Passed, zero errors; warnings retained | static |
| Patch whitespace | `git diff --check` | Passed | static |

The focused regressions include 14 cache cases, four preloader cases and four
Robolectric tests of the actual ViewModel using synthetic browse responses.
The synchronous navigation test queues new coroutines before asserting cards;
other cases cover stale refresh, explicit refresh, pagination, late responses,
memory/account invalidation, concurrency across cancelled batches, disabled
preloading and stopping after service errors.

## Not verified and limits

No emulator or physical device is attached. Instrumentation compilation does not
establish device latency, visible pixels or live-service/account behavior. Cold
artwork and a never-prepared category still need their first network response.
No APK is assembled or published by this change.

Open PR #55 also changes `ComposeHost.kt` and `RustRepository.kt`; the feed-card
appearance call may need to combine its scrolling condition with this category
animation bypass. Draft integration PR #48 overlaps these files and the Android
dependency file. This PR does not depend on either branch.

## For the plan and parity tables

M9: cached video/Music category pages display synchronously without entrance
delays, with bounded stale-page refresh and broader two-request background
preparation during mini-player playback. Cache/concurrency and actual-ViewModel
regressions qualify fixture/unit behavior; physical and live category-switch
latency remain open.
