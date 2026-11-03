<!--
Title: `type(M#): imperative summary`, at most 72 characters.
  type  = feat | fix | perf | test | docs | chore | ci | refactor | style | revert
  M#    = the milestone ID(s) from RUST_REWRITE_PLAN.md, e.g. M9 or M5/M9
  e.g.  fix(M9): retry covers that fail to load

Fill every section, then delete these HTML comments (they end up in the squash commit).
Write for a reader who has not seen the conversation or the diff. Rules: docs/AGENT_PR_RULES.md
-->

## What changed
<!-- 1-3 sentences. Lead with what a user, or the next developer, can now do or no longer suffers.
     Not a file list: the diff already shows that. -->

## Why
<!-- The problem or request. Link the issue, the milestone, or quote the owner's instruction with its date. -->

## Verification
<!-- Only checks you actually ran in this branch, with the result. Name the strongest evidence level reached:
     fixture/unit < emulator < physical device < live service/account. Never claim a higher level than you ran. -->

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | |
| `cargo test --workspace --locked` | |

**Evidence level reached:** fixture / emulator / physical device / live

## Not verified
<!-- What you did not run or could not run, and why (no device, no account, network blocked, ...).
     Known limits and follow-ups. Write "Nothing known" only if you looked. -->

## Merge notes
<!-- Keep these labels; CI reads them. -->
- **Affects:** desktop / Android / shared engine / CI-release / docs only
- **Change note:** `docs/changes/YYYY-MM-DD-slug.md` (required for feat, fix and perf)
- **Depends on:** none
- **Overlaps with:** none <!-- other open PRs touching the same files, by number -->
- **Auto-merge:** yes <!-- or "no, because ..." (needs owner decision, draft, unverified live behaviour) -->
- **Version, CI or release files touched:** no
