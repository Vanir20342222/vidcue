import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location(
    "prepare_preview_release", Path(__file__).with_name("prepare-preview-release.py")
)
preview = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preview)

VERSION = "0.2.0-alpha.12-mobile.11"
RUNS = {"Android": "111", "Rust": "222", "Secret scan": "333"}


class PreviewPublicationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.tested = self.root / "tested"
        self.tested.mkdir()
        self.apk = self.tested / "app-debug.apk"
        self.revision = "a" * 40
        self.fixture()

    def fixture(self, source=None, version=VERSION, abis=("arm64-v8a", "x86_64")):
        dex = (source or self.revision) + " " + version
        with zipfile.ZipFile(self.apk, "w") as archive:
            archive.writestr("classes.dex", dex)
            for abi in abis:
                archive.writestr(f"lib/{abi}/libvidcue_mobile.so", b"fixture")
            archive.writestr("assets/notices/inventory.json", "{}")
            archive.writestr("assets/vidcue-LICENSE", "MIT")
        digest = hashlib.sha256(self.apk.read_bytes()).hexdigest()
        self.apk.with_name("SHA256-and-checks.txt").write_text(
            "Verified signatures, ZIP alignment, both Rust ABIs, 16 KiB ELF segments and notices.\n"
            f"{digest}  /build/app-debug.apk\n"
        )

    def prepare(self, version=VERSION, runs=RUNS, changes="", base="android-x-mobile.10"):
        preview.prepare(
            self.tested, self.root / "publish", self.revision, runs, version, base, changes
        )

    def test_published_apk_is_byte_identical_to_tested_artifact(self):
        self.prepare()
        published = list((self.root / "publish").glob("*.apk"))
        self.assertEqual([p.name for p in published], [f"vidcue-preview-{VERSION}.apk"])
        self.assertEqual(published[0].read_bytes(), self.apk.read_bytes())
        digest = hashlib.sha256(self.apk.read_bytes()).hexdigest()
        self.assertEqual(
            (self.root / "publish" / "SHA256SUMS.txt").read_text(),
            f"{digest}  vidcue-preview-{VERSION}.apk\n",
        )

    def test_altered_apk_is_rejected(self):
        with self.apk.open("ab") as stream:
            stream.write(b"changed")
        with self.assertRaisesRegex(ValueError, "CI inspection"):
            self.prepare()
        self.assertFalse((self.root / "publish").exists())

    def test_wrong_source_is_rejected_even_with_valid_checksum(self):
        self.fixture(source="b" * 40)
        with self.assertRaisesRegex(ValueError, "checked main"):
            self.prepare()
        self.assertFalse((self.root / "publish").exists())

    def test_wrong_version_is_rejected_even_with_valid_checksum(self):
        self.fixture(version="0.2.0-alpha.12-mobile.8")
        with self.assertRaisesRegex(ValueError, "requested Preview version"):
            self.prepare()
        self.assertFalse((self.root / "publish").exists())

    def test_missing_abi_is_rejected(self):
        self.fixture(abis=("arm64-v8a",))
        with self.assertRaises(KeyError):
            self.prepare()

    def test_ambiguous_artifacts_are_rejected(self):
        other = self.tested / "other"
        other.mkdir()
        (other / self.apk.name).write_bytes(self.apk.read_bytes())
        with self.assertRaisesRegex(ValueError, "exactly one"):
            self.prepare()

    def test_unsafe_versions_and_runs_are_rejected(self):
        for version in ("", "../x", "a b", "-leading", "x" * 65, "v\nmalicious"):
            with self.subTest(version=version):
                with self.assertRaisesRegex(ValueError, "version"):
                    self.prepare(version=version)
        with self.assertRaisesRegex(ValueError, "run set"):
            self.prepare(runs={**RUNS, "Rust": "0"})
        with self.assertRaisesRegex(ValueError, "run set"):
            self.prepare(runs={"Android": "1"})
        self.assertFalse((self.root / "publish").exists())

    def test_notes_state_what_the_build_is_and_is_not(self):
        self.prepare(changes="Merge PR #36: watch-page recommendations in Up next (Android)\n")
        notes = (self.root / "publish" / "notes.md").read_text()
        self.assertIn("CI debug-signed build, not the stable channel", notes)
        self.assertIn("dev.vidcue.android.preview", notes)
        self.assertIn("It was not rebuilt or re-signed", notes)
        for name, run in RUNS.items():
            self.assertIn(f"[{name}](https://github.com/Vanir20342222/vidcue/actions/runs/{run})", notes)
        self.assertIn("- Merge PR #36: watch-page recommendations in Up next (Android)", notes)
        self.assertIn("under-1000 ms startup is not claimed", notes)
        self.assertIn(f"`{self.revision}`", notes)
        report = (self.root / "publish" / "APK-inspection.txt").read_text()
        self.assertTrue(report.startswith(f"Source revision: {self.revision}\n"))

    def test_change_list_is_bounded_and_printable(self):
        text = "\n".join(f"change {i}\x07\x1b[31m" for i in range(200)) + "\n\n" + "x" * 500
        listed = preview.changes_markdown(text).splitlines()
        self.assertEqual(len(listed), preview.MAX_CHANGES)
        self.assertTrue(all(line.startswith("- ") and line.isprintable() for line in listed))
        self.assertEqual(preview.changes_markdown("\n \n"), "- No merged changes were listed.")
        self.assertLessEqual(len(preview.changes_markdown("y" * 500)), 162)


if __name__ == "__main__":
    unittest.main()
