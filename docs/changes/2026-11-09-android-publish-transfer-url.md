# Android local-publish transfer URL accepts any HTTPS host (M9)

- **Date:** 2026-11-09
- **PR:** #<number> <!-- add once the PR exists -->
- **Milestone:** M9
- **Platforms:** CI-release / Android
- **Status:** fixture/unit verified

## What it does

The locally signed APK publisher no longer requires the temporary transfer URL
to point at `.oaiusercontent.com`. Any HTTPS URL is accepted, so beta, RC and
final releases can be published from any file host without touching the
workflow for every release. Non-HTTPS URLs, credentials in the URL, non-443
ports, fragments, `localhost` and IP-literal hosts are still refused, and
redirects must stay HTTPS. The validation moved into `release_channel.py` so
the dispatch and handoff-branch paths cannot drift apart.

Transfer integrity is unchanged: the publisher still pins the caller-supplied
SHA-256, verifies the stable signing certificate, checks package, version code
ordering against every published release, the dex source-revision marker, ABI
and 16 KiB alignment. The host allowlist was never one of those guarantees.

## Evidence

| Check | Command or procedure | Result | Level |
| --- | --- | --- | --- |
| Publication boundary tests | `python3 tools/android/test_verify_local_release.py` | 6 tests OK | fixture/unit verified |
| Handoff manifest tests | `python3 tools/android/test_publish_request.py` | 3 tests OK | fixture/unit verified |
| Channel rule tests | `python3 tools/android/test_release_channel.py` | OK | fixture/unit verified |
| Signing build tests | `python3 tools/android/test_sign_local.py` | 3 tests OK | fixture/unit verified |
| Workflow syntax | `yaml.safe_load` on `android-publish-local.yml` | parses | fixture/unit verified |

## Not verified and limits

- A real end-to-end dispatch of the publish workflow with a non-`.oaiusercontent.com`
  URL was not run; that requires a signed beta.8 APK and Actions execution.
- `localhost` and IP-literal hosts are refused as transfer hygiene; a public
  HTTPS file host is expected.

## For the plan and parity tables

No plan or parity row changes: publication tooling only, no app behavior change.
