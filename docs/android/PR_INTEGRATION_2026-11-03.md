# M9: integrate all open PRs and prepare Preview delivery

The owner requested resolving all conflicts and CI failures, merging all PR
features, and publishing a downloadable APK on 2026-11-03. This supersedes the
earlier PR-only/draft instructions in PRs #32, #47 and #48. Their historical
failed measurements remain in their evidence documents.

The combined source includes main's onboarding, account screen, watch-page
recommendations and desktop parity, plus:

- #32: shared media connections, parallel bounded byte prefetch, foreground
  priority, early activity-owned player-view preparation and intent cancellation.
- #47: separate copy-request/completion clocks and service cleanup waits that
  tolerate the expected preceding playback error.
- #48: SurfaceView rendering, Compose surface synchronization and full-window
  compositor evidence that includes opaque covers.

Conflicts were resolved by preserving the prepared-video lifecycle together
with recommendation/playlist context, `context_entry`/`context_skip`, resume and
manual-queue behavior. The duplicate video-probe readback constant was removed.
The strict completion deadlines and genuine output/playing requirements remain.

The legacy emulator TextureView renderer workaround disabled detached/placeholder
surfaces and forced codec recreation on every surface change. SurfaceView now
uses Media3's normal surface lifecycle, allowing codec setup to overlap layout.
The existing emulator-only software decoder selection remains; physical devices
retain normal codec selection. Runtime qualification is required for this change.

## Checks and delivery

Local combined-source checks: 390 workspace Rust tests passed, 31 explicit
ignores; formatting and strict workspace Clippy passed. The 15 vendored mobile
playback tests, offline CI media setup checks, native solver vectors and 11
signing/publication helper tests passed. Android compilation/JVM/lint and hosted
emulator results are recorded below when completed.

`android-preview-*` release tags now publish only the exact APK from successful
Android/Rust/Secret scan runs for the tagged current-main commit. Publication
checks the source marker and inspection checksum, includes both ABIs/notices,
and retains the tested signature. It performs no rebuild or signing.

The default Preview version is `0.2.0-alpha.12-mobile.11`, code 11. The permanent
stable signing identity is outside this cloud workspace, so this delivery uses
`dev.ytui.android.preview` and a CI debug identity, separate from the stable app.
Do not uninstall an existing installation to work around a signing mismatch.

Physical Tab S9/Nothing Phone acceptance, public YouTube latency and account
behavior, audible output and stable-key update qualification remain open. M9
remains in progress; offline CI cannot close those gates.
