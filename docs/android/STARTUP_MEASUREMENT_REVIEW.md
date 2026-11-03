# Startup evidence follow-up (M9)

This follow-up is separate from PR32 so parallel pushes do not cancel its native
qualification. It changes instrumentation and evidence only. The application
performance implementation stays in PR32; neither PR is authorized for merging.

## Request and completion are different measurements

PixelCopy is asynchronous. Android 15's
[RenderProxy implementation](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android15-release/libs/hwui/renderthread/RenderProxy.cpp)
posts `readback().copySurfaceInto(...)` onto the render thread's queue. The Java
request timestamp does not establish when its pixels were sampled; the API does
not supply a buffer-presentation timestamp.

Keep request time and callback completion separate. A completed RGB window copy
is a conservative confirmation bound, including observer overhead, rather than
an exact physical-display timestamp. The original strict startup checks use
completion, real RGB content and active playback within their unchanged budget.
A two-second late-readback allowance is diagnostic only: pixels returned after
the startup deadline cannot qualify it. Reports now retain `copy_request_ms`
alongside confirmation and observer duration. No request-time backdating is used.
This can reject a start that a physical display showed earlier; such a case needs
better timestamped evidence or a device measurement, not an assumed sample time.

## Retained current application evidence

Source `eb1ef69`, workflow merge `1307261`, Android run
[41598899034](https://github.com/Vanir20342222/ytui/actions/runs/41598899034)
was cancelled by a parallel push to PR32. Two complete shards passed; partial
shards retain **30 passes and two failures** overall. This is not a completed
57-case qualification. Rust and both secret scans passed.

| Synthetic path | Visible and playing | Completed window evidence | Result |
| --- | ---: | ---: | --- |
| Cold card, Best progressive | 511 ms | 694 ms | Strict one-second check passed |
| Prefetched card, Best progressive | 456 ms | 557 ms | Strict one-second check passed |
| Uncached paired indexed | 1044 ms | 1432 ms | Original two-second check passed; above new goal |
| Prefetched paired indexed | 1179 ms | 1178 ms | Strict one-second check failed |

The paired indexed failure includes actual playing delay; it cannot be explained
away by screenshot overhead. Frame initialization and physical YouTube latency
remain open. The idle view's lifecycle/recreation check passed in the completed
shard. Remaining cancelled source-selection and cover tests need a complete run.

Artifacts: shards0–3 `13213368703`, `13214350914`, `13213428539`, `13214043030`;
APK `13213912545`. The downloaded APK independently passes signature, both ABIs,
16 KiB alignment and notices, with SHA256
`a28011d27e452cab645c83ababab2c8f728a0499d3a910196207ae548ffa5dfd`.
It is a Preview build, not a release or universal subsecond qualification.

## Cleanup wait correction

One retained failure occurred in `PlaybackPerformanceTest` setup, before its
socket test body. The preceding navigation test intentionally caused a playback
error. The shared wait helper treated that stale error as a new failure while
service destruction was still in progress; logs show session release immediately
afterwards. Cleanup now ignores that prior playback error while still requiring
actual service destruction within the same 60-second timeout. Normal playback
waits retain immediate failure on a real playback error. No service teardown or
networking assertion is removed.

Local test-APK compilation and lint precede fresh native qualification. Real
Tab S9 Best-quality timings remain required for the user's subsecond YouTube goal;
loopback media and callback timing cannot substitute for that report.

## First independent run

Source `2cc8404`, workflow merge `13fd645`, run
[41601395656](https://github.com/Vanir20342222/ytui/actions/runs/41601395656)
completed all 57 cases: 52 pass, four fail and one optional public skip. Rust and
secret scans passed. Rapid selection/transport feedback and the previously
aborted socket-recovery test passed. Strict cold-card usable852/window1008 ms
and prepared indexed no usable start by the deadline remain failures. The
rejected-FGS test hit the same stale-error race in its own destruction wait;
that targeted wait now also ignores its expected error while requiring actual
closure within the unchanged timeout. The service does not crash in this run.
One catalog cover remained a placeholder despite the integrated retry changes;
the failure-only native probe decoded it successfully and recorded per-cover
request counts. That image-loading issue remains open.

Artifacts: shards0–3 `13214532139`, `13215071859`, `13214463054`, `13214552767`;
APK `13215001069`. These results do not qualify the subsecond goal.
