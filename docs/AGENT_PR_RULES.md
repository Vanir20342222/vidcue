# Rules for agents that push pull requests

Read this before opening or updating a PR. It exists because one week of parallel PRs took about
12 hours to merge: each merge made the next PR stale, nearly every PR edited the same few documents,
and CI failures were fixed by pushing more commits. The owner's decisions (2026-11-03) are built in.

Where this file and a task prompt disagree, the task prompt wins, but say so in the PR.

## 1. Before you start

1. `git fetch origin main` and branch from the tip of `main`. Never branch from another unmerged PR
   unless the task says so (then set **Depends on** and open the PR as a draft).
2. List open PRs (`list_pull_requests`) and look at the files they change. If one overlaps with
   yours, say so in **Overlaps with**, keep your edits to the shared lines minimal, and do not
   reformat, reorder or move code in those files.
3. One topic per PR. A PR that mixes a feature, a refactor and CI changes is split. Stay near
   400 changed lines of non-test code; if it must be larger, say why in **What changed**.
4. Conflict hotspots (each was edited 13 to 45 times in a week): `ComposeHost.kt`, `CatalogUiTest.kt`,
   `PlaybackService.java`, `RustRepository.kt`, `FocusedPlayer.kt`, `apps/desktop/src/app.rs`.
   Put new code in a new file and touch the hotspot only to wire it in.

## 2. What you may and may not change

| Area | Rule |
| --- | --- |
| Source, tests, `docs/changes/` | Yes, within your topic |
| Shared evidence and plan docs: `RUST_REWRITE_PLAN.md`, `docs/VALIDATION.md`, `docs/android/VALIDATION.md`, PARITY, PLAN and README indexes, `docs/REMAINING_WORK.md` | **No.** Write a change note instead (section 5). Needs the `shared-docs` label from the owner |
| `.github/workflows/**`, `tools/ci/**`, the PR template | **No**, unless the task is a CI task. Needs the `ci-change` label |
| Version numbers (`YTUI_ANDROID_VERSION_*` in `apps/android/app/build.gradle`, `version` in `Cargo.toml`) | **No.** The owner bumps versions at release. Needs `release-change` |
| Release and signing workflows, keystores, secrets | Never. Do not dispatch release workflows |
| `vendor/**` | Only for a task that is about the vendored crate |
| Tests | Never skip, delete, loosen, add retries to, or widen a budget of a test to get green. Fix the cause, or stop and report (section 6) |
| `Cargo.lock` | Only when you changed a dependency |

The `PR hygiene` check enforces the rows that name a label. Do not add the labels yourself.

## 3. Checks before you push

Run what your change can break, and paste what you ran in **Verification**.

- Rust or shared engine: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`.
- Android Kotlin or Java: compile the app and the test APK (`apps/android/gradlew -p apps/android :app:compileDebugKotlin :app:compileDebugAndroidTestKotlin`) when the SDK is available; otherwise say it was not compiled.
- Docs only: nothing to build. CI skips Android and Rust for documentation-only changes.

Tests stay offline and deterministic. Never read real cookies or sign in to a real account.
Never commit cookies, tokens, signed stream URLs or private feed fixtures.

## 4. Commits and staying current

- Conventional subjects with the milestone: `fix(M9): retry covers that fail to load`.
- **Do not merge `main` into your branch just to be current.** Auto-merge and the branch rules do
  not require an up-to-date branch. Merge `main` only when GitHub reports a conflict, and then once.
- Never rebase or force-push a branch that already has a PR with review activity.
- Do not push several fix-up commits for one problem. Reproduce, fix, run the check, push once.

## 5. Evidence goes in one new file

For `feat`, `fix` and `perf` PRs add `docs/changes/YYYY-MM-DD-slug.md` from
[docs/changes/TEMPLATE.md](changes/TEMPLATE.md). Put the plan and parity sentences you would have
written into the shared documents under "For the plan and parity tables". The owner collates them.

Evidence vocabulary (AGENTS.md): implemented, fixture/unit verified, emulator verified,
physical-device verified, live verified. A compile, a fixture or a green emulator run never
establishes a live-service, physical-device or stable-release claim.

## 6. When CI is red or the PR goes stale

1. Open the failing job's log first. Decide which of these it is:
   - **Your change broke it.** Reproduce locally if you can, fix once, push once.
   - **Infrastructure** (runner lost, download failed, emulator did not boot). Re-run the failed
     jobs once. Do not change the code or the workflow.
   - **A startup timing budget.** These are advisory on PRs and enforced nightly. A PR failure
     with a budget message means something hung: treat it as your change.
2. **Stop after two failed attempts on the same job.** Post one PR comment with the failing job,
   the log excerpt that matters, what you tried and what you suspect, then end your turn and tell
   the owner. Do not keep pushing.
3. A conflict with `main`: merge `origin/main` once. If the conflict is mechanical (adjacent
   edits, imports, a doc), resolve it and say what you did. If both sides changed the behaviour of
   the same function, stop and ask the owner which wins.
4. Never: push an empty commit or close and reopen to re-trigger CI, change a workflow to skip a
   job, or resolve someone else's PR conflicts.

## 7. Opening the PR

- Use [the PR template](../.github/pull_request_template.md). Delete the HTML comments. The
  description becomes the squash commit message.
- Title: `type(M#): summary`, at most 72 characters.
- Open as **draft** if the work is unfinished, depends on an unmerged PR, or needs an owner
  decision. Otherwise open it ready for review.
- Write for a reader who has not seen your conversation:
  - **What changed**: the visible effect, 1 to 3 sentences, not a file list.
  - **Why**: the request or defect, with a link or a dated owner quote.
  - **Verification**: only checks you ran, with results. Name the evidence level reached.
  - **Not verified**: what you could not run and the known limits. "Nothing known" only if you looked.
  - **Merge notes**: fill every label; they are machine-checked.
- Use plain statements. No marketing words ("robust", "seamless"), no claims you did not test, no
  history of your attempts. Put design reasoning in the change note, not the description.
- **Auto-merge:** write `yes` only if nothing needs an owner decision and no live, device or
  account check is still outstanding for the claim in the title. Otherwise write
  `no, because ...`. Do not enable auto-merge yourself unless the task prompt says to
  (`enable_pr_auto_merge`); the owner enables it with one click.

## 8. After you push

- Subscribe to the PR only if the task says to watch it. When subscribed, follow section 6.
- When the PR is merged, the work is finished. Follow-up work is a new branch from the new `main`,
  never new commits on the merged branch.
- End your turn with: the PR link, a one-line status (draft/ready, CI state, auto-merge yes/no),
  what was and was not verified, and any question for the owner.
