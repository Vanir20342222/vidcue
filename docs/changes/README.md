# Change notes

One file per pull request. This replaces appending sections to the shared evidence and plan
documents, which made every parallel PR conflict (`docs/android/VALIDATION.md` was edited 29 times
and `RUST_REWRITE_PLAN.md` 22 times in a week, mostly "keep both sections" merges).

- Copy [TEMPLATE.md](TEMPLATE.md) to `docs/changes/YYYY-MM-DD-short-slug.md` in the same PR.
- Required for `feat`, `fix` and `perf` PRs (checked by the `PR hygiene` workflow); optional otherwise.
- A note records what this PR did and the evidence for it. It does not edit other documents.
- Do not edit `RUST_REWRITE_PLAN.md`, `docs/VALIDATION.md`, `docs/android/VALIDATION.md`, the PARITY,
  PLAN or README indexes. The owner folds notes into them when preparing a release:
  `tools/collate-change-notes.sh 2026-11-03` prints every note on or after that date, oldest first.
- Notes are history. Never rewrite an older note; add a new one that supersedes it and link back.
- Keep AGENTS.md's evidence vocabulary: *implemented*, *fixture/unit verified*, *emulator verified*,
  *physical-device verified*, *live verified*. Do not claim a level you did not run.
