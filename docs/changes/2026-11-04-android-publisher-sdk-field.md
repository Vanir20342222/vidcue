# Read current Android minimum-SDK metadata (M9)

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android release tooling
- **Status:** fixture and signed-artifact verified

## What it does

The publisher accepted only `sdkVersion` in aapt2 badging. The installed 35.0.0
tools report `minSdkVersion`, so the first beta handoff passed source/checksum/
certificate/package/version verification but stopped before publication. Accept
both documented field names while still requiring a numeric minimum SDK. Missing
or unknown fields remain errors. No package, signing key, version or app behavior
changes.

## Evidence

The failure was reproduced by running the complete publisher verifier against the
actual signed beta APK, with only the transfer replaced by local file reads. The
same real artifact verifies after the parsing fix. Boundary tests cover both field
names and reject a missing/unknown field. All 30 release-helper tests pass.

## Not verified and limits

Final publication is repeated from qualified main. Physical/live and actual
beta.1 → beta.2 update acceptance remain separate from APK metadata verification.

## For the plan and parity tables

- Android release tooling: publisher reads both current `minSdkVersion` and legacy
  `sdkVersion` aapt2 output without changing any source/signature/version gate.
