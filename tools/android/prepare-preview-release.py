#!/usr/bin/env python3
"""Prepare an inspected CI Preview APK for publication, without changing its signature.

The published APK is the byte-identical artifact that the exact-main Android workflow built,
inspected and ran its emulator checks against. Nothing here rebuilds or re-signs it.
"""

import hashlib
import os
from pathlib import Path
import re
import shutil
import zipfile

VERSION = re.compile(r"[A-Za-z0-9][A-Za-z0-9.+_-]{0,63}")
REVISION = re.compile(r"[0-9a-f]{40}")
RUN = re.compile(r"[1-9][0-9]*")
CHECKS = ("Android", "Rust", "Secret scan")
MAX_CHANGES = 40


def tag_for(version):
    return f"android-preview-{version}"


def changes_markdown(text):
    """Bullet list from repository-controlled commit subjects, bounded and printable."""
    lines = []
    for raw in text.splitlines():
        line = "".join(c for c in raw if c.isprintable()).strip()[:160]
        if line:
            lines.append(f"- {line}")
        if len(lines) == MAX_CHANGES:
            break
    return "\n".join(lines) if lines else "- No merged changes were listed."


def notes(version, revision, runs, base, changes):
    owner = "https://github.com/Vanir20342222/ytui"
    links = ", ".join(f"[{name}]({owner}/actions/runs/{runs[name]})" for name in CHECKS)
    return (
        f"Android **development Preview** `{version}`, built by GitHub Actions from `main` at "
        f"`{revision[:12]}`.\n\n"
        "**This is a CI debug-signed build, not the stable channel.** It installs as "
        "**vidcue Preview** (`dev.vidcue.android.preview`) next to the stable **vidcue** app "
        "(`dev.vidcue.android.stable`). It does not update the stable app and keeps its own "
        "settings, account and library, so sign in and choose folders again. Every CI build "
        "uses its own debug certificate, so Android will normally refuse to install one "
        "Preview over another. If that happens, export anything you want to keep before "
        "replacing it; never uninstall a wanted installation just to bypass the check. "
        "In-place stable updates need the owner's locally held signing key and are published "
        "separately.\n\n"
        "Download the `.apk` below and open it from Files. Android 8+, ARM64 and x86-64.\n\n"
        f"## Changes since `{base}`\n\n{changes_markdown(changes)}\n\n"
        "## Verification\n\n"
        f"Source `{revision}` passed the exact-main checks: {links}. This is the exact APK "
        "from that Android run, including its emulator checks. It was not rebuilt or "
        "re-signed. Check it against `SHA256SUMS.txt`; `APK-inspection.txt` records the "
        "signature, alignment, both-ABI and notice inspection.\n\n"
        "**Not established by these offline checks:** Galaxy Tab S9 / Nothing Phone (3a) "
        "acceptance, live YouTube startup latency (under-1000 ms startup is not claimed), "
        "audible output, account and network behavior on a physical device, and updates "
        "under the permanent signing key.\n\n"
        f"[Validation record]({owner}/blob/{revision}/docs/android/VALIDATION.md)\n"
    )


def prepare(tested, output, revision, runs, version, base="unknown", changes=""):
    if not REVISION.fullmatch(revision):
        raise ValueError("Invalid source revision")
    if not VERSION.fullmatch(version):
        raise ValueError("Invalid preview version")
    if set(runs) != set(CHECKS) or not all(RUN.fullmatch(str(runs[name])) for name in CHECKS):
        raise ValueError("Invalid check run set")
    candidates = list(tested.rglob("app-debug.apk"))
    if len(candidates) != 1:
        raise ValueError("Expected exactly one tested Preview APK")
    apk = candidates[0]
    report = apk.with_name("SHA256-and-checks.txt").read_text()
    digest = hashlib.sha256(apk.read_bytes()).hexdigest()
    hashes = re.findall(r"^([0-9a-f]{64})\s+\S*app-debug\.apk$", report, re.M)
    if hashes != [digest] or "Verified signatures, ZIP alignment, both Rust ABIs" not in report:
        raise ValueError("APK does not match its CI inspection")
    with zipfile.ZipFile(apk) as archive:
        for abi in ("arm64-v8a", "x86_64"):
            archive.getinfo(f"lib/{abi}/libvidcue_mobile.so")
        archive.getinfo("assets/notices/inventory.json")
        archive.getinfo("assets/vidcue-LICENSE")
        dex = [
            archive.read(name)
            for name in archive.namelist()
            if re.fullmatch(r"classes[0-9]*\.dex", name)
        ]
    # BuildConfig constants are in the dex string pool: they tie the file to its source and name.
    if not any(revision.encode() in data for data in dex):
        raise ValueError("APK source differs from the checked main revision")
    if not any(version.encode() in data for data in dex):
        raise ValueError("APK version differs from the requested Preview version")
    output.mkdir()
    name = f"vidcue-preview-{version}.apk"
    shutil.copyfile(apk, output / name)
    (output / "SHA256SUMS.txt").write_text(f"{digest}  {name}\n")
    (output / "APK-inspection.txt").write_text(f"Source revision: {revision}\n" + report)
    (output / "notes.md").write_text(notes(version, revision, runs, base, changes))


if __name__ == "__main__":
    changes_file = Path(os.environ.get("YTUI_PREVIEW_CHANGES", "changes.txt"))
    prepare(
        Path("tested"),
        Path("publish"),
        os.environ["YTUI_PREVIEW_SOURCE"],
        {
            "Android": os.environ["YTUI_PREVIEW_ANDROID_RUN"],
            "Rust": os.environ["YTUI_PREVIEW_RUST_RUN"],
            "Secret scan": os.environ["YTUI_PREVIEW_SECRETS_RUN"],
        },
        os.environ["YTUI_PREVIEW_VERSION"],
        os.environ.get("YTUI_PREVIEW_BASE", "unknown"),
        changes_file.read_text() if changes_file.is_file() else "",
    )
