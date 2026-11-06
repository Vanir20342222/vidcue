#!/usr/bin/env python3
"""Rules that keep every published Android APK installable as an update of the beta.

Android only replaces an installed app in place when the new APK has
- the same package name (`dev.vidcue.android.stable`),
- the same signing certificate (the owner's stable key), and
- a strictly higher version code.

The locally signed publisher checks all three against the releases already on GitHub
and writes a machine-readable marker into the release notes. The app's update check
reads only releases that carry this marker. Debug-signed CI Preview APKs can never
update the stable app, so once a marker release exists, the Preview publisher refuses
to run.

Usage:
  release_channel.py code VERSION_NAME          # version code for a beta/rc/final name
  release_channel.py check-preview RELEASES.json [VERSION_NAME]
"""

import json
from pathlib import Path
import re
import sys

PACKAGE = "dev.vidcue.android.stable"
CERT_SHA256 = "d3c9001eb3b22b31549f515f2d976000d3280ae8d5d0fc517bd03be946ddfd20"
KEYSTORE_FILENAME = "vidcue-beta.jks"
MARKER = "vidcue-android-update"
MARKER_RE = re.compile(r"<!--\s*" + MARKER + r"\s+(\{.*?\})\s*-->", re.S)
# Owner-signed releases before the marker existed state their code in the notes.
LEGACY_CODE_RE = re.compile(r"\bcode \*\*([1-9][0-9]{0,9})\*\*")


def load_releases(path):
    """Releases from `gh api --paginate`, which prints one JSON array per page."""
    text = Path(path).read_text()
    decoder = json.JSONDecoder()
    releases, index = [], 0
    while True:
        while index < len(text) and text[index].isspace():
            index += 1
        if index >= len(text):
            return releases
        page, index = decoder.raw_decode(text, index)
        if not isinstance(page, list):
            raise ValueError("Unexpected GitHub releases response")
        releases.extend(page)


def marker_of(release):
    """The update marker of one release, or None."""
    match = MARKER_RE.search(release.get("body") or "")
    if not match:
        return None
    data = json.loads(match.group(1))
    return data if isinstance(data, dict) else None


def stable_releases(releases):
    """Owner-signed stable releases: `android-<version>` tags, not CI Previews or drafts."""
    return [
        r for r in releases
        if not r.get("draft")
        and str(r.get("tag_name", "")).startswith("android-")
        and not str(r.get("tag_name", "")).startswith("android-preview-")
    ]


def highest_published_code(releases, package=PACKAGE):
    """Highest version code already published for `package` (0 when none)."""
    codes = [0]
    for release in stable_releases(releases):
        marker = marker_of(release)
        if marker is not None:
            if marker.get("package") == package:
                codes.append(int(marker.get("versionCode", 0)))
            continue
        codes += [int(c) for c in LEGACY_CODE_RE.findall(release.get("body") or "")]
    return max(codes)


def beta_started(releases):
    """True once any published release carries the update marker."""
    return any(marker_of(r) is not None for r in stable_releases(releases))


class UpdateOrderError(ValueError):
    """The version code would not install over an already published release."""


def require_update(releases, code, package=PACKAGE, cert_sha256=CERT_SHA256):
    """Refuse a version code that would not install over every earlier release."""
    highest = highest_published_code(releases, package)
    for release in stable_releases(releases):
        previous = marker_of(release)
        if previous is not None and previous.get("package") == package:
            if previous.get("certSha256") != cert_sha256:
                raise UpdateOrderError(
                    "The signing certificate must match every published release in this channel"
                )
    if int(code) <= highest:
        raise UpdateOrderError(
            f"Version code {code} must be higher than every published release ({highest}); "
            "Android refuses to install a lower or equal code as an update"
        )


def marker(*, version_name, version_code, apk, sha256, cert_sha256, min_sdk, package=PACKAGE):
    """Invisible HTML comment that the app's update check reads from the release notes."""
    for value, pattern in [
        (version_name, r"[A-Za-z0-9][A-Za-z0-9.+_-]{0,63}"),
        (apk, r"[A-Za-z0-9][A-Za-z0-9.+_-]{0,127}\.apk"),
        (sha256, r"[0-9a-f]{64}"),
        (cert_sha256, r"[0-9a-f]{64}"),
        (package, r"[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+"),
    ]:
        if not re.fullmatch(pattern, str(value)):
            raise ValueError(f"Invalid update marker value: {value!r}")
    data = {
        "package": package,
        "versionCode": int(version_code),
        "versionName": version_name,
        "apk": apk,
        "sha256": sha256,
        "certSha256": cert_sha256,
        "minSdk": int(min_sdk),
    }
    return f"<!-- {MARKER} {json.dumps(data, separators=(',', ':'))} -->"


VERSION_RE = re.compile(r"(\d{1,3})\.(\d{1,2})\.(\d{1,2})(?:-(beta|rc)\.([1-9]\d?))?")


def version_code(version_name):
    """Version code for `MAJOR.MINOR.PATCH`, `…-beta.N` (N 1-49) or `…-rc.N` (N 1-48).

    MAJOR*1,000,000 + MINOR*10,000 + PATCH*100 + stage, where stage is N for beta.N,
    50+N for rc.N and 99 for the final release. So 0.3.0-beta.1 → 30001, 0.3.0-rc.1 → 30051,
    0.3.0 → 30099 and 0.3.1-beta.1 → 30101: every later version gets a higher code, and all
    of them are above the alpha codes (10-15) already published.
    """
    match = VERSION_RE.fullmatch(version_name)
    limit = {"beta": 49, "rc": 48}.get(match.group(4) if match else None, 0)
    if not match or (match.group(5) and int(match.group(5)) > limit):
        raise ValueError(
            "Use MAJOR.MINOR.PATCH, MAJOR.MINOR.PATCH-beta.N (N 1-49) or MAJOR.MINOR.PATCH-rc.N (N 1-48)"
        )
    major, minor, patch = (int(match.group(i)) for i in (1, 2, 3))
    stage = {"beta": 0, "rc": 50}.get(match.group(4), 99)
    if match.group(5):
        stage += int(match.group(5))
    return major * 1_000_000 + minor * 10_000 + patch * 100 + stage


def main(argv):
    if len(argv) == 3 and argv[1] == "code":
        print(version_code(argv[2]))
        return
    if len(argv) in (3, 4) and argv[1] == "check-preview":
        if beta_started(load_releases(argv[2])):
            sys.exit(
                "The beta channel has started: every published APK must update the installed "
                "beta, and CI debug-signed Preview APKs cannot. Publish an owner-signed build "
                "with 'Publish locally signed Android APK' instead (docs/android/SIGNING.md)."
            )
        if len(argv) == 4 and VERSION_RE.fullmatch(argv[3]):
            sys.exit(
                "Beta, release-candidate and final APKs require the owner's stable signing "
                "identity, including the first beta. Publish an owner-signed build with "
                "'Publish locally signed Android APK' instead (docs/android/SIGNING.md)."
            )
        print("No beta release yet; a CI Preview release is still allowed.")
        return
    sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
