"""Synthetic publication checks; no signing key, account or external server used."""

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch, Mock
import zipfile

spec = importlib.util.spec_from_file_location(
    "verify_release", Path(__file__).with_name("verify-local-release.py")
)
verify = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify)


class PublicationBoundary(unittest.TestCase):
    def test_transfer_rejects_non_https_credentials_and_internal_hosts(self):
        for url in [
            "http://example.com/a", "https://127.0.0.1/a", "https://localhost/a",
            "https://[::1]/a", "https://u:p@example.com/a", "https://example.com:8443/a",
            "https://example.com/a#fragment",
        ]:
            with self.subTest(url=url), self.assertRaises(ValueError):
                verify.release_channel.transfer_url(url)
        for url in ["https://a.oaiusercontent.com/a?synthetic=token",
                    "https://example.com/apk?synthetic=token"]:
            with self.subTest(url=url):
                self.assertEqual(verify.release_channel.transfer_url(url), url)

    def run_fixture(self, *, wrong_digest=False, wrong_source=False,
                    wrong_certificate=False, debuggable=False, published_code=3, handoff=False,
                    sdk_field="sdkVersion"):
        source = "1" * 40
        buffer = io.BytesIO()
        with zipfile.ZipFile(buffer, "w") as archive:
            archive.writestr("classes.dex", ("2" * 40 if wrong_source else source).encode())
        payload = buffer.getvalue()
        env = {
            "YTUI_VERSION_NAME": "1.0-preview", "YTUI_VERSION_CODE": "4",
            "YTUI_APK_SHA256": "0" * 64 if wrong_digest else hashlib.sha256(payload).hexdigest(),
            "GITHUB_SHA": source, "YTUI_APK_URL": "https://a.oaiusercontent.com/fixture",
            "ANDROID_HOME": "/synthetic-sdk",
        }
        if handoff:
            env.update(GITHUB_SHA="3" * 40, YTUI_RELEASE_SOURCE=source)
        response = io.BytesIO(payload)
        response.status = 200
        opener = Mock()
        opener.open.return_value = response
        cert = "0" * 64 if wrong_certificate else verify.STABLE_CERT
        signature = f"Signer #1 certificate SHA-256 digest: {cert}\n"
        badging = (
            "package: name='dev.vidcue.android.stable' versionCode='4' versionName='1.0-preview' "
            f"platformBuildVersionName='15'\n{sdk_field}:'26'\n"
        )
        if debuggable:
            badging += "application-debuggable\n"
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as folder:
            try:
                os.chdir(folder)
                releases = Path(folder) / "releases.json"
                # Two pages, as `gh api --paginate` prints them.
                releases.write_text(
                    json.dumps([{"tag_name": "android-0.9", "body": f"code **{published_code}**."}])
                    + "\n" + json.dumps([{"tag_name": "android-preview-9", "body": "code **99**"}])
                )
                env["YTUI_PUBLISHED_RELEASES"] = str(releases)
                with patch.dict(os.environ, env), patch.object(
                    verify.urllib.request, "build_opener", return_value=opener
                ), patch.object(verify.subprocess, "check_output", side_effect=[signature, badging]):
                    verify.main()
                notes = Path("publish/notes.md").read_text()
                marker = verify.release_channel.marker_of({"body": notes})
                self.assertEqual(marker["versionCode"], 4)
                self.assertEqual(marker["package"], "dev.vidcue.android.stable")
                self.assertEqual(marker["apk"], "vidcue-1.0-preview.apk")
                self.assertEqual(marker["certSha256"], verify.STABLE_CERT)
                self.assertEqual(marker["sha256"], env["YTUI_APK_SHA256"])
                self.assertEqual(marker["minSdk"], 26)
                self.assertIn(source, Path("publish/APK-inspection.txt").read_text())
            finally:
                os.chdir(previous)

    def test_valid_transfer_is_traceable(self):
        self.run_fixture()

    def test_both_android_tool_minimum_sdk_field_names_are_supported(self):
        for name in ("sdkVersion", "minSdkVersion"):
            with self.subTest(name=name):
                self.run_fixture(sdk_field=name)
        with self.assertRaises(ValueError):
            self.run_fixture(sdk_field="unknownSdkField")

    def test_metadata_only_handoff_still_requires_the_qualified_apk_source(self):
        self.run_fixture(handoff=True)
        with self.assertRaises(ValueError):
            self.run_fixture(handoff=True, wrong_source=True)

    def test_version_code_must_exceed_every_published_release(self):
        for code in (4, 5):
            with self.subTest(published=code), self.assertRaises(
                verify.release_channel.UpdateOrderError
            ):
                self.run_fixture(published_code=code)

    def test_tampering_other_sources_keys_and_debug_packages_are_rejected(self):
        for option in ["wrong_digest", "wrong_source", "wrong_certificate", "debuggable"]:
            with self.subTest(option=option), self.assertRaises(ValueError):
                self.run_fixture(**{option: True})


if __name__ == "__main__":
    unittest.main()
