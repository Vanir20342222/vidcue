#!/usr/bin/env python3
"""Transfer a finished public-release APK using a temporary metadata-only branch.

Only APK metadata and a short-lived HTTPS transfer URL travel in its commit message.
Signing keys/passwords never enter Git or Actions. The publisher masks the URL and
deletes the handoff branch afterward; the permanent release tag points at source only.
"""

import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_channel  # noqa: E402


def validate(data, source):
    if not isinstance(data, dict) or set(data) != {
        "version_name", "version_code", "sha256", "apk_url", "source"
    }:
        raise ValueError("Unexpected release manifest")
    if not re.fullmatch(r"[0-9a-f]{40}", source) or data["source"] != source:
        raise ValueError("Source mismatch")
    version = data["version_name"]
    if (not isinstance(version, str) or type(data["version_code"]) is not int or
        release_channel.version_code(version) != data["version_code"]):
        raise ValueError("Version/code mismatch")
    if not re.fullmatch(r"[0-9a-f]{64}", data["sha256"]):
        raise ValueError("Invalid checksum")
    address = data["apk_url"]
    if not isinstance(address, str) or any(c in address for c in "\r\n"):
        raise ValueError("Invalid transfer URL")
    release_channel.transfer_url(address)
    return data


def git(*args, **kwargs):
    return subprocess.check_output(["git", *args], text=True, **kwargs).strip()


def create(data):
    source = git("rev-parse", "HEAD")
    validate(data, source)
    if git("status", "--porcelain"):
        raise ValueError("Signing source must be clean")
    branch = f"publish-android-{data['version_name']}-{secrets.token_hex(6)}"
    commit = git("commit-tree", git("rev-parse", "HEAD^{tree}"), "-p", source,
                 input=json.dumps(data))
    return branch, commit


def read_branch(branch, commit):
    if not re.fullmatch(r"publish-android-[A-Za-z0-9.+_-]+-[0-9a-f]{12}", branch):
        raise ValueError("Invalid handoff branch")
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or git("rev-parse", "HEAD") != commit:
        raise ValueError("Handoff checkout mismatch")
    parents = git("rev-list", "--parents", "-n", "1", commit).split()
    if len(parents) != 2:
        raise ValueError("Exactly one source parent required")
    source = parents[1]
    data = validate(json.loads(git("show", "-s", "--format=%B", commit)), source)
    if git("rev-parse", commit + "^{tree}") != git("rev-parse", source + "^{tree}"):
        raise ValueError("Handoff must not change source files")
    if not re.fullmatch(r"publish-android-" + re.escape(data["version_name"]) + r"-[0-9a-f]{12}", branch):
        raise ValueError("Branch/version mismatch")
    return data


def export(data, path):
    print("::add-mask::" + data["apk_url"].replace("%", "%25"))
    values = {
        "YTUI_APK_URL": data["apk_url"], "YTUI_APK_SHA256": data["sha256"],
        "YTUI_VERSION_CODE": data["version_code"], "YTUI_VERSION_NAME": data["version_name"],
        "YTUI_RELEASE_SOURCE": data["source"],
    }
    with Path(path).open("a") as file:
        for name, value in values.items():
            file.write(f"{name}={value}\n")


def main(argv):
    if len(argv) == 3 and argv[1] == "create":
        print(*create(json.loads(Path(argv[2]).read_text())))
    elif len(argv) == 3 and argv[1] == "read-branch":
        export(read_branch(argv[2], os.environ["GITHUB_SHA"]), os.environ["GITHUB_ENV"])
    else:
        raise ValueError("Use create MANIFEST.json or read-branch BRANCH")


if __name__ == "__main__":
    try:
        main(sys.argv)
    except Exception:
        # Errors must never echo the temporary URL or its query token.
        sys.exit("Invalid Android release handoff; no release published.")
