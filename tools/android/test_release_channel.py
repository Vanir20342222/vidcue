"""Offline checks for the rules that keep every stable APK an in-place update."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "release_channel", Path(__file__).with_name("release_channel.py")
)
channel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(channel)

CERT = "b" * 64


def marked(tag, code, package=channel.PACKAGE, draft=False):
    body = "Notes\n\n" + channel.marker(
        version_name=tag.removeprefix("android-"),
        version_code=code,
        apk=f"vidcue-{code}.apk",
        sha256="a" * 64,
        cert_sha256=CERT,
        min_sdk=26,
        package=package,
    )
    return {"tag_name": tag, "body": body, "draft": draft}


class ReleaseChannel(unittest.TestCase):
    def test_marker_round_trips_and_rejects_unsafe_values(self):
        release = marked("android-0.3.0-beta.1", 300100)
        data = channel.marker_of(release)
        self.assertEqual(data["versionCode"], 300100)
        self.assertEqual(data["certSha256"], CERT)
        self.assertIn("<!-- vidcue-android-update {", release["body"])
        for bad in [{"apk": "../x.apk"}, {"sha256": "z" * 64}, {"version_name": "1 2"},
                    {"apk": "x.zip"}, {"package": "Bad.Package"}]:
            args = dict(version_name="1.0", version_code=2, apk="a.apk", sha256="a" * 64,
                        cert_sha256=CERT, min_sdk=26)
            args.update(bad)
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                channel.marker(**args)

    def test_highest_code_uses_markers_and_legacy_notes_but_not_previews_or_drafts(self):
        releases = [
            {"tag_name": "android-0.2.0-alpha.12-mobile.10", "body": "code **10**. Android 8+"},
            {"tag_name": "android-preview-0.2.0-alpha.12-mobile.15", "body": "code **15**"},
            marked("android-0.3.0-beta.1", 300100),
            marked("android-9.9.9", 999999999, draft=True),
            marked("android-other", 999999998, package="dev.example.other"),
        ]
        self.assertEqual(channel.highest_published_code(releases), 300100)
        self.assertEqual(channel.highest_published_code(releases[:2]), 10)
        self.assertEqual(channel.highest_published_code([]), 0)

    def test_require_update_needs_a_strictly_higher_code(self):
        releases = [marked("android-0.3.0-beta.1", 300100)]
        channel.require_update(releases, 300101)
        for code in (300100, 300099, 1):
            with self.subTest(code=code), self.assertRaises(channel.UpdateOrderError):
                channel.require_update(releases, code)

    def test_preview_releases_stop_once_the_beta_exists(self):
        legacy = [{"tag_name": "android-0.2.0-alpha.12-mobile.10", "body": "code **10**"}]
        self.assertFalse(channel.beta_started(legacy))
        self.assertTrue(channel.beta_started(legacy + [marked("android-0.3.0-beta.1", 300100)]))
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "releases.json"
            path.write_text(json.dumps(legacy))
            channel.main(["release_channel.py", "check-preview", str(path)])
            path.write_text(json.dumps(legacy) + "\n" + json.dumps([marked("android-0.3.0-beta.1", 3)]))
            self.assertEqual(len(channel.load_releases(path)), 2)
            with self.assertRaises(SystemExit) as stop:
                channel.main(["release_channel.py", "check-preview", str(path)])
            self.assertIn("beta channel has started", str(stop.exception))


class VersionCodes(unittest.TestCase):
    def test_first_beta_cannot_be_published_through_the_debug_preview_channel(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "releases.json"
            path.write_text("[]")
            for version in ["0.3.0-beta.1", "0.3.0-rc.1", "0.3.0"]:
                with self.subTest(version=version), self.assertRaises(SystemExit):
                    channel.main(["release_channel.py", "check-preview", str(path), version])
            channel.main(["release_channel.py", "check-preview", str(path), "0.2.0-alpha.12-mobile.15"])

    def test_codes_increase_through_beta_rc_and_final_and_exceed_the_alphas(self):
        ordered = ["0.3.0-beta.1", "0.3.0-beta.2", "0.3.0-beta.49", "0.3.0-rc.1",
                   "0.3.0-rc.48", "0.3.0", "0.3.1-beta.1", "0.3.1", "0.4.0-beta.1",
                   "1.0.0-beta.1", "1.0.0", "10.99.99"]
        codes = [channel.version_code(v) for v in ordered]
        self.assertEqual(codes, sorted(codes))
        self.assertEqual(len(set(codes)), len(codes))
        self.assertEqual(channel.version_code("0.3.0-beta.1"), 30001)
        self.assertEqual(channel.version_code("0.3.0"), 30099)
        self.assertGreater(codes[0], 15)
        self.assertLessEqual(codes[-1], 2100000000)

    def test_other_names_are_refused(self):
        for bad in ["0.3", "0.3.0-alpha.1", "0.3.0-beta.50", "0.3.0-rc.49", "0.3.0-beta.0", "v0.3.0",
                    "0.2.0-alpha.12-mobile.10", "0.100.0"]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                channel.version_code(bad)


if __name__ == "__main__":
    unittest.main()
