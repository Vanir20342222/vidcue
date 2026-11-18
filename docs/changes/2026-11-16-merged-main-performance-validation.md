# M9 merged-main performance validation

- Date: 2026-11-16
- Source: `4ca8522c680733b89a4df8a83fe7cafc88bad44a`
- Scope: host validation after merging PRs #168 and #169; physical rerun blocked

Both PRs are merged. The previously failed #168 Android app job passed on its
rerun. On the exact combined main checkout, the following command completed
successfully (exit 0, 10m21s):

```sh
YTUI_ANDROID_REVISION=$(git rev-parse HEAD) apps/android/gradlew -p apps/android \
  --no-daemon :app:testDebugUnitTest :app:lintDebug
```

The JVM result XML records 313 tests, zero failures/errors/skips. Main Kotlin and
Java compilation and Android app/instrumentation/unit-test lint analysis passed.
This check did not build fresh JNI libraries or perform a device/live run.
Host log: `/tmp/vidcue-merged-main-host-checks.log` (not a Git artifact).

At the final check, exact-main secret scanning had passed, the Android APK job
was building both Rust ABIs/checking the Android host, and the Rust release-build
job had passed with its remaining jobs queued. These pending hosted jobs are not
reported as passes.

## Original target audit

The physical measurements below predate this combined-main rerun and come from
[the physical-device change note](2026-11-16-galaxy-tab-locale-fixes.md) and
[the performance evidence](2026-11-15-android-performance-targets.md).

- Video below 500 ms: two prefetched public YouTube cases met it; cold starts
  remained above it. Films also remained above it. Broad target not achieved.
- Initial populated feed below 1000 ms: first anonymous Music Home remained
  above it; a repeat met it. Anonymous video Home returned no recommendations,
  which cannot count as a populated feed. Broad target not achieved.
- Music around 100 ms: physical AudioTrack playout observations remained above
  it. Active-playing state alone does not establish actual output. Not achieved.
- Maximum practical download throughput: deterministic chunk integrity and local
  throughput checks exist; representative live-CDN throughput is still missing.
  No universal or maximum-speed claim is qualified.
- Exact merged-main tablet acceptance: not run. Earlier physical successes and
  host tests cannot substitute for it after the upstream UI integration.

## External blocker

The explicitly selected SM-X710 tablet is absent from `adb devices`. Its
advertised wireless-debugging endpoint repeatedly refuses connections. This
condition persisted across three consecutive goal continuations. Existing
pairing was reused; no credentials were requested or read. Continuing the
physical/live qualification requires the owner to enable Wireless debugging
and provide the current connection endpoint. No new timing assertion, quality
reduction, account access, signing change or production release was introduced.
