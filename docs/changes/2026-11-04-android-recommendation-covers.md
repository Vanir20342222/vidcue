# M9: automatic recommendation covers

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android
- **Status:** fixture/unit verified

## What it does

Android's `Artwork` already starts loading when a recommendation becomes visible,
but its artwork-availability check skips cards with no thumbnail metadata. These
cards keep the placeholder regardless of retries. Add a public YouTube cover URL
derived from a valid watch URL when a video/song has neither a thumbnail nor sized
variants. The availability check, cache identity and JNI request use the same
resolved artwork metadata. The original playback/queue/history JSON is preserved.

Supplied artwork and the Thumbnails setting retain their existing behavior. The
fallback accepts YouTube watch, Music, mobile, short-link and Shorts URLs, checks
the video ID, and excludes collections, stations and unrelated origins. Loading
continues through the existing bounded cache and cancellable retry path.

## Evidence

- Compiled `ArtworkSource.kt` and `ArtworkSourceTest.kt` with Kotlin 2.1.21 (the
  Android project's compiler version), then ran JUnit 4.13.2 directly: **6 tests
  passed**. These cover missing/supplied artwork, sized variants, URL forms,
  empty/null metadata, malformed/foreign URLs, collections and stations.
- `git diff --check`: passed.

## Not verified and limits

The workspace has no Android SDK. APK compilation, emulator/device rendering and
live YouTube CDN delivery were not verified. This addresses the missing-metadata
path; it does not establish the cause of every placeholder in the supplied
screenshot or resolve unavailable/blocked image origins. M9 remains a preview.

## For the plan and parity tables

M9 Android: recommendations with missing artwork metadata derive a public
YouTube cover URL and use the existing automatic loader/cache. Six offline JVM
regressions pass; APK, device and live-CDN verification remain pending.
