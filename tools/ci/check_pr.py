#!/usr/bin/env python3
"""Cheap, deterministic pull-request hygiene gate (see docs/AGENT_PR_RULES.md).

Reads the pull_request event from $GITHUB_EVENT_PATH and the change under test from
`git diff HEAD^1 HEAD`. It never calls the network and never runs repository code.
Exit 0 means the PR may proceed; exit 1 prints every problem with the fix.
Events that are not pull requests (merge queue, push) pass immediately: the PR already passed.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys

TITLE_RE = re.compile(
    r"^(feat|fix|perf|test|docs|chore|ci|refactor|style|revert)"
    r"(\([A-Za-z0-9/,. _-]+\))?!?: \S.{3,}$"
)
TITLE_MAX = 72
NOTE_TYPES = {"feat", "fix", "perf"}
REQUIRED_SECTIONS = ["What changed", "Why", "Verification", "Not verified", "Merge notes"]
REQUIRED_MERGE_LABELS = ["Affects", "Change note", "Auto-merge"]

# Shared files that every parallel PR used to edit, producing the "keep both sections"
# conflicts. Evidence now goes in docs/changes/<date>-<slug>.md instead.
SHARED_DOCS = {
    "RUST_REWRITE_PLAN.md",
    "docs/VALIDATION.md",
    "docs/PARITY.md",
    "docs/REMAINING_WORK.md",
    "docs/README.md",
    "docs/android/VALIDATION.md",
    "docs/android/PARITY.md",
    "docs/android/PLAN.md",
    "docs/android/README.md",
}
CI_PREFIXES = (".github/workflows/", "tools/ci/", ".github/pull_request_template.md")
VERSION_FILES = ("apps/android/app/build.gradle", "Cargo.toml")
VERSION_LINE_RE = re.compile(r"^[+-](?:.*YTUI_ANDROID_VERSION|version = )")

LABEL_FOR = {
    "shared-docs": "edits a shared evidence/plan document",
    "ci-change": "edits workflows or CI tooling",
    "release-change": "changes a version number",
}


def strip_comments(text: str) -> str:
    return re.sub(r"<!--.*?-->", "", text, flags=re.S)


def sections(body: str) -> dict[str, str]:
    """Map '## Heading' to its text with HTML comments removed."""
    found: dict[str, str] = {}
    current = None
    for line in strip_comments(body).splitlines():
        match = re.match(r"^##\s+(.+?)\s*$", line)
        if match:
            current = match.group(1)
            found[current] = ""
        elif current is not None:
            found[current] += line + "\n"
    return found


def meaningful(text: str) -> bool:
    """Non-empty, and more than the untouched template rows or bare punctuation."""
    cleaned = re.sub(r"[|\-\s*`]", "", text)
    return len(cleaned) >= 3


def check_body(title: str, body: str) -> list[str]:
    problems: list[str] = []
    match = TITLE_RE.match(title)
    if not match:
        problems.append(
            f"Title {title!r} must look like `type(M#): summary` "
            "(types: feat fix perf test docs chore ci refactor style revert)."
        )
    if len(title) > TITLE_MAX:
        problems.append(f"Title is {len(title)} characters; keep it at most {TITLE_MAX}.")
    found = sections(body or "")
    for name in REQUIRED_SECTIONS:
        if name not in found:
            problems.append(f"Missing section `## {name}` (copy it from the PR template).")
        elif not meaningful(found[name]):
            problems.append(f"Section `## {name}` is empty; fill it in or write why it does not apply.")
    notes = found.get("Merge notes", "")
    for label in REQUIRED_MERGE_LABELS:
        if not re.search(rf"\*\*{re.escape(label)}:\*\*\s*\S", notes):
            problems.append(f"`## Merge notes` needs a filled `**{label}:**` line.")
    if re.search(r"\*\*Affects:\*\*[^\n]*desktop / Android / shared engine", notes):
        problems.append("`**Affects:**` still lists every option; keep only the ones this PR touches.")
    verification = found.get("Verification", "")
    if "fixture / emulator / physical device / live" in verification:
        problems.append("`**Evidence level reached:**` still lists every option; keep the one you reached.")
    if "Evidence level reached" in verification and not re.search(
        r"Evidence level reached:\*\*\s*(fixture|unit|emulator|physical device|live)", verification, re.I
    ):
        problems.append("`**Evidence level reached:**` must be fixture, emulator, physical device or live.")
    return problems


def check_files(
    title: str,
    labels: set[str],
    status_lines: list[str],
    version_diff: str,
) -> list[str]:
    problems: list[str] = []
    paths = []
    for line in status_lines:
        parts = line.split("\t")
        if len(parts) >= 2:
            paths.append((parts[0][0], parts[-1]))
    changed = {path for _, path in paths}

    shared = sorted(changed & SHARED_DOCS)
    if shared and "shared-docs" not in labels:
        problems.append(
            "Edits shared document(s) " + ", ".join(shared) + ". Put evidence and status in a new "
            "docs/changes/<date>-<slug>.md instead, or have the owner add the `shared-docs` label."
        )
    ci = sorted(path for path in changed if path.startswith(CI_PREFIXES))
    if ci and "ci-change" not in labels:
        problems.append(
            "Edits CI/workflow files (" + ", ".join(ci[:4]) + "). Only do this when the task asks for it, "
            "and have the owner add the `ci-change` label."
        )
    if version_diff and "release-change" not in labels:
        problems.append(
            "Changes a version number. Agents never bump versions; the owner does it at release time "
            "(label `release-change` if this is intended)."
        )
    match = TITLE_RE.match(title)
    kind = match.group(1) if match else ""
    if kind in NOTE_TYPES:
        notes = [
            path for _, path in paths
            if re.fullmatch(r"docs/changes/[^/]+\.md", path) and path not in {"docs/changes/README.md", "docs/changes/TEMPLATE.md"}
        ]
        if not notes:
            problems.append(
                f"A `{kind}` PR needs a change note: copy docs/changes/TEMPLATE.md to "
                "docs/changes/<date>-<slug>.md and fill it in."
            )
    return problems


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def main() -> int:
    event_path = os.environ.get("GITHUB_EVENT_PATH")
    event = json.load(open(event_path)) if event_path else {}
    pr = event.get("pull_request")
    if not pr:
        print("Not a pull_request event; nothing to check.")
        return 0
    title = pr.get("title") or ""
    body = pr.get("body") or ""
    labels = {label["name"] for label in pr.get("labels", [])}
    problems = check_body(title, body)
    problems += check_files(
        title,
        labels,
        git("diff", "--name-status", "HEAD^1", "HEAD").splitlines(),
        "\n".join(
            line for line in git("diff", "-U0", "HEAD^1", "HEAD", "--", *VERSION_FILES).splitlines()
            if VERSION_LINE_RE.match(line)
        ),
    )
    if problems:
        print("PR hygiene failed. Fix these, then edit the PR (this check reruns on edit):\n")
        for problem in problems:
            print(f"  - {problem}")
        print("\nRules and template: docs/AGENT_PR_RULES.md, .github/pull_request_template.md")
        return 1
    print("PR hygiene passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
