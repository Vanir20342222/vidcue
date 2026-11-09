"""Offline source/version/transfer boundaries for APK publication handoffs."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("publish_request.py").resolve()
spec = importlib.util.spec_from_file_location("publish_request", SCRIPT)
request = importlib.util.module_from_spec(spec)
spec.loader.exec_module(request)


def manifest(source="1" * 40):
    return dict(version_name="0.3.0-beta.1", version_code=30001, sha256="a" * 64,
                apk_url="https://transfer.example.test/apk?synthetic=token", source=source)


class PublishRequest(unittest.TestCase):
    def test_requires_exact_source_version_code_checksum_and_transfer_url(self):
        good = manifest()
        self.assertEqual(request.validate(good, "1" * 40), good)
        for changes in [dict(source="2" * 40), dict(version_code=15), dict(sha256="z" * 64),
                        dict(apk_url="http://example.test/app.apk"),
                        dict(apk_url="https://127.0.0.1/app.apk"),
                        dict(apk_url="https://example.test:8443/app.apk"),
                        dict(apk_url=good["apk_url"] + "\nINJECT=value"),
                        dict(apk_url="https://u:p@example.test/a")]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                request.validate(good | changes, "1" * 40)

    def test_handoff_round_trips_and_changed_source_or_wrong_commit_fails(self):
        previous = Path.cwd()
        with tempfile.TemporaryDirectory() as folder:
            try:
                os.chdir(folder)
                subprocess.run(["git", "init", "-q"], check=True)
                subprocess.run(["git", "config", "user.name", "Fixture"], check=True)
                subprocess.run(["git", "config", "user.email", "fixture@example.invalid"], check=True)
                Path("source").write_text("synthetic only")
                subprocess.run(["git", "add", "source"], check=True)
                subprocess.run(["git", "commit", "-qm", "fixture"], check=True)
                source = request.git("rev-parse", "HEAD")
                data = manifest(source)
                branch, commit = request.create(data)
                subprocess.run(["git", "checkout", "-q", "--detach", commit], check=True)
                self.assertEqual(request.read_branch(branch, commit), data)
                with self.assertRaises(ValueError):
                    request.read_branch(branch, "0" * 40)
                Path("source").write_text("changed source must fail")
                subprocess.run(["git", "add", "source"], check=True)
                changed = request.git("write-tree")
                bad = request.git("commit-tree", changed, "-p", source, input=json.dumps(data))
                subprocess.run(["git", "checkout", "-q", "--detach", bad], check=True)
                with self.assertRaises(ValueError):
                    request.read_branch(branch, bad)
            finally:
                os.chdir(previous)

    def test_url_is_masked_before_export_and_errors_do_not_echo_it(self):
        data = manifest()
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "environment"
            with contextlib.redirect_stdout(io.StringIO()) as log:
                request.export(data, output)
            self.assertTrue(log.getvalue().startswith("::add-mask::"))
            self.assertIn("YTUI_VERSION_CODE=30001\n", output.read_text())
            bad = Path(folder) / "bad.json"
            bad.write_text(json.dumps(data | dict(version_code=1)))
            result = subprocess.run([sys.executable, str(SCRIPT), "create", str(bad)],
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn("synthetic=token", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
