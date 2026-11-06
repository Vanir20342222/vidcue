# Permanent Android beta signing channel (M9)

- **Date:** 2026-11-04
- **Milestone:** M9
- **Platforms:** Android, CI-release, docs
- **Status:** fixture/unit verified; signed release verification in progress

## What it does

The owner clarified that beta.1 starts a new identity and only subsequent releases
must update it. This supersedes AGENTS.md's old application-ID preservation rule
and the earlier need to recover the alpha key. The release task authorizes the
signing/package/workflow changes in this PR.

The permanent package is `dev.vidcue.android.stable`, with the certificate pinned
in the shared release policy. Its one-time key was generated privately and backed
up to owner-only Drive with encryption to the owner's existing SSH public key.
The encrypted readback matches the local archive byte-for-byte. Signing secrets
never enter Git or Actions. Old alpha apps and their data remain separate.

The publisher checks earlier beta-channel markers as well as the source pin, so
all later beta/RC/final APKs must keep the package/key and increase the code. A
temporary metadata-only branch handoff can start the existing verified-APK publisher when
no release-dispatch API is exposed. It carries only the public-release APK's
expiring read URL/metadata, masks that URL in Actions and deletes the temporary branch.

## Evidence

- Python boundaries: 28 tests pass, including certificate continuity and handoff
  parent/tree/source/version/URL validation; no network or real signing secrets in tests.
- Private backup: owner-only Drive metadata and byte-for-byte readback verified.
- Android app and instrumentation-source compile, 133 JVM tests (0 failures/errors/skips)
  and lint pass locally. No Android app logic was changed beyond the application ID.
- Signed APK/source inspection and publication evidence belongs to the release run.

## Not verified and limits

Signing does not establish physical update or live-service acceptance. Actual
beta.1 → beta.2 installation/data retention remains a device check. Ordinary builds
and the publisher never generate a replacement signing identity.

## For the plan and parity tables

- M9: beta.1 establishes `dev.vidcue.android.stable` with a privately backed-up
  permanent key. All later beta/RC/final releases update that beta; compatibility
  with old alphas is not required. Physical update acceptance remains open.
