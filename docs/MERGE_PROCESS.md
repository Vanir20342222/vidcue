# Merging and releasing: what was slow and how it is set up now

Owner-facing. Agent-facing rules are in [AGENT_PR_RULES.md](AGENT_PR_RULES.md). Written 2026-11-03
after the 2-3 November merge run, which took about 12 hours from the first open PR to the last merge,
plus roughly 15 minutes more before the Preview APK could be released.

## What the history and CI runs show

Measured from the repository history and the Actions runs of 2-3 November 2026.

| Finding | Evidence |
| --- | --- |
| Merging was strictly serial. PRs #34-#43 were opened together at 20:15 on 2 Nov; they merged between 20:43 and 09:14 the next day. | PR timestamps |
| Every merge made the next PR stale, so each needed "merge main", a second CI run and often conflict fixes. About 12 "Merge main into ... branch" commits in this window. | `git log` |
| A clean Android run takes 12-15 minutes (APK job 5.7-6.9 min, then four emulator shards 2.7-6.6 min, the slowest decides). Rust takes 3-4 minutes. | Run 41609467400, 41607058605, Rust runs 250-257 |
| Of the 14 completed Android runs I inspected (#162-#175), 7 failed and 2 were cancelled. Commit messages attribute several failures to startup timing assertions on the software emulator ("Indexed pixels: 2001"). I did not read every failed log. | Actions runs |
| Most conflicts were in shared documents: `docs/android/VALIDATION.md` edited 29 times, `RUST_REWRITE_PLAN.md` 22, several plan documents 12-16, plus `ComposeHost.kt` 45. | `git log --name-only` |
| A merge to `main` ran Android and Rust again, and the release workflow requires green runs for the exact `main` commit. Each merge therefore added another full cycle before a release. | `android-preview-release.yml` |
| Branch pushes ran CI twice (push and pull_request) and the secret scan ran on every branch push with full history. | `rust.yml`, `security.yml` |

## What changed in this repository

| Change | Effect |
| --- | --- |
| `docs/changes/` note per PR; the PR check rejects edits to the shared plan and evidence documents | Removes most "keep both sections" conflicts |
| `PR hygiene` workflow and `.github/pull_request_template.md` | Description, title, change note and forbidden-file rules are checked in seconds, before the 15-minute build |
| CI path filter (`tools/ci/should-run.sh`): documentation-only PRs skip Android and Rust; desktop-only PRs skip Android; Android-only PRs skip Rust | A docs PR is green in about a minute instead of 15. `main`, nightly and manual runs always run everything |
| Aggregate jobs `Android gate` and `Rust gate` | One stable required check per workflow that also passes when a build was skipped on purpose |
| Startup timing budgets advisory on PRs, enforced by a nightly run (03:17 UTC) and on demand (`timing: enforce`) | A 2001 ms frame on a shared runner no longer blocks a merge; hangs still fail (6 s ceiling). Measured times stay in the evidence files |
| Rust CI no longer runs on branch pushes; secret scan only on `main` pushes, PRs and the merge queue | Fewer duplicate runs competing for runners |
| `merge_group` trigger on Android, Rust, secret scan and hygiene | Ready for a merge queue (see below) |
| Removed `android-preview.yml` (one-off publish with a hard-coded commit, run id and artifact id from 28 Oct) and `native-playback.yml` (ran only for branch `fix/native-playback`) | Fewer confusing workflows. Nothing referenced them; `git revert` restores them |

Not verified here: the workflow edits and the Kotlin test change were not run on GitHub or an
emulator from this environment. The first PR that carries them is their test. `should-run.sh`, the
hygiene checker and the run-device-tests.sh syntax were run locally.

## Settings only the owner can change

### Merge mode

**Merge queue** tests each PR on top of the latest `main` and merges it without a manual
"update branch". GitHub documents it for repositories owned by an organization (public repositories
on any plan, private ones on Enterprise Cloud). This repository belongs to a personal account, and I
could not confirm that a personal repository can enable it. If **Require merge queue** does not appear in
the ruleset, use Mode B. Moving the repository to a free organization is the way to get Mode A.

**Mode B: auto-merge without "up to date" (works on personal repositories)**

1. Settings → General → Pull Requests: enable **Allow auto-merge**, **Allow squash merging** with the
   default message "Pull request title and description", and **Automatically delete head branches**.
2. Settings → Rules → Rulesets → new branch ruleset for `main`:
   - Require a pull request before merging (0 required approvals if you are the only reviewer).
   - Require status checks: `Android gate`, `Rust gate`, `secrets` (the job in "Secret scan"), `PR hygiene`.
   - **Do not** tick "Require branches to be up to date before merging". That setting is what forced
     every PR to merge main and rebuild after each neighbour merged.
3. Enable auto-merge on a PR (button on the PR page, or `enable_pr_auto_merge`). It merges as soon as
   its own four checks pass.

The trade-off: a PR is tested against the `main` it branched from, not the latest. Two PRs that are
each green can break each other. The run on `main` after each merge, which the release workflow
already requires, catches that. With the conflict hotspots and shared documents reduced this should
be rare; if it happens, fix forward in a new PR.

**Mode A: merge queue (organization-owned repository)**

Same settings as Mode B, plus in the ruleset enable **Require merge queue** with merge method
Squash, build concurrency 3, group size min 1 / max 3, status check timeout 30 minutes, and
"Only merge non-failing pull requests". `merge_group` triggers are already in the workflows.
Hosted runners are limited, so keep build concurrency at 3 or fewer: each entry uses 5 Android and
3 Rust jobs.

### Labels

Create `shared-docs`, `ci-change` and `release-change`. Only you add them. They let a PR edit shared
documents, workflows or version numbers when that is intended (for example the release PR).

## Releasing the Android Preview

Unchanged mechanics, fewer steps in the way:

1. Land the PRs (auto-merge). Wait for the CI run of the final `main` commit (about 15 minutes).
2. If the version should change, merge one small PR with the `release-change` label that bumps
   `YTUI_ANDROID_VERSION_NAME` and `_CODE` defaults in `apps/android/app/build.gradle`. Agents never do this.
3. Actions → **Release verified Android Preview** → Run workflow on `main` with the same
   `version_name`. It requires successful Android, Rust and Secret scan runs for the exact `main`
   commit and publishes the tested APK without rebuilding.
4. Before writing release or plan notes, run `tools/collate-change-notes.sh <since-date>` and paste
   the "For the plan and parity tables" sections into `RUST_REWRITE_PLAN.md`, PARITY and VALIDATION.

## Not done, worth considering

- The four emulator shards are unbalanced (2:44 to 6:33 in one run). Re-balancing by measured test
  duration could save 2-3 minutes per run. It needs per-test timings from the evidence files.
- `main` re-runs the full Android build for a commit the PR already tested. It is needed today because
  the release workflow and artifact name key on the exact `main` commit. With a merge queue the release
  workflow could accept the `merge_group` run instead.
- The version bump in `build.gradle` and the release dispatch could be one workflow that takes the
  version as an input, removing the release PR.
- `ComposeHost.kt` is 3,620 lines and was edited 45 times in a week. Splitting it by screen would
  remove the largest source of code conflicts.
