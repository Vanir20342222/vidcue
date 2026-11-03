import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_pr  # noqa: E402

GOOD_BODY = """
<!-- comment -->
## What changed
Covers that fail to load are retried with backoff.

## Why
Owner report 2026-11-02: blank covers stay blank.

## Verification
| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 389 passed |

**Evidence level reached:** emulator

## Not verified
Physical Galaxy Tab S9 run.

## Merge notes
- **Affects:** Android
- **Change note:** `docs/changes/2026-11-03-cover-retry.md`
- **Depends on:** none
- **Auto-merge:** yes
"""


class BodyTests(unittest.TestCase):
    def test_good_body_passes(self):
        self.assertEqual(check_pr.check_body("fix(M9): retry covers that fail to load", GOOD_BODY), [])

    def test_template_left_untouched_fails(self):
        template = (Path(__file__).resolve().parents[2] / ".github/pull_request_template.md").read_text()
        problems = check_pr.check_body("fix(M9): retry covers", template)
        self.assertTrue(any("What changed" in p for p in problems), problems)
        self.assertTrue(any("Why" in p for p in problems), problems)
        self.assertTrue(any("Not verified" in p for p in problems), problems)

    def test_bad_title(self):
        self.assertTrue(check_pr.check_body("Update stuff", GOOD_BODY))
        self.assertTrue(check_pr.check_body("fix(M9): " + "x" * 80, GOOD_BODY))

    def test_missing_merge_label(self):
        body = GOOD_BODY.replace("- **Auto-merge:** yes\n", "")
        self.assertTrue(any("Auto-merge" in p for p in check_pr.check_body("fix(M9): retry covers", body)))

    def test_bad_evidence_level(self):
        body = GOOD_BODY.replace("emulator", "great")
        self.assertTrue(any("Evidence level" in p for p in check_pr.check_body("fix(M9): retry covers", body)))

    def test_unchosen_template_options_fail(self):
        body = GOOD_BODY.replace("**Affects:** Android", "**Affects:** desktop / Android / shared engine / CI-release / docs only")
        self.assertTrue(any("Affects" in p for p in check_pr.check_body("fix(M9): retry covers", body)))
        body = GOOD_BODY.replace("emulator", "fixture / emulator / physical device / live")
        self.assertTrue(any("Evidence level" in p for p in check_pr.check_body("fix(M9): retry covers", body)))

    def test_empty_body(self):
        self.assertTrue(check_pr.check_body("fix(M9): retry covers", ""))


class FileTests(unittest.TestCase):
    NOTE = ["A\tdocs/changes/2026-11-03-cover-retry.md", "M\tapps/android/app/src/main/Foo.kt"]

    def test_note_present(self):
        self.assertEqual(check_pr.check_files("fix(M9): retry covers", set(), self.NOTE, ""), [])

    def test_note_missing_for_fix(self):
        problems = check_pr.check_files("fix(M9): retry covers", set(), ["M\tapps/android/Foo.kt"], "")
        self.assertTrue(any("change note" in p for p in problems))

    def test_note_not_required_for_docs_or_test(self):
        self.assertEqual(check_pr.check_files("test(M9): timing", set(), ["M\tapps/android/FooTest.kt"], ""), [])

    def test_template_is_not_a_note(self):
        problems = check_pr.check_files("feat(M5): x feature", set(), ["M\tdocs/changes/TEMPLATE.md"], "")
        self.assertTrue(any("change note" in p for p in problems))

    def test_shared_doc_needs_label(self):
        files = self.NOTE + ["M\tdocs/android/VALIDATION.md"]
        self.assertTrue(any("shared document" in p for p in check_pr.check_files("fix(M9): retry covers", set(), files, "")))
        self.assertEqual(check_pr.check_files("fix(M9): retry covers", {"shared-docs"}, files, ""), [])

    def test_workflow_needs_label(self):
        files = self.NOTE + ["M\t.github/workflows/android.yml"]
        self.assertTrue(check_pr.check_files("fix(M9): retry covers", set(), files, ""))
        self.assertEqual(check_pr.check_files("fix(M9): retry covers", {"ci-change"}, files, ""), [])

    def test_version_needs_label(self):
        diff = "+def requestedVersionName = System.getenv('YTUI_ANDROID_VERSION_NAME') ?: 'x'"
        self.assertTrue(check_pr.check_files("fix(M9): retry covers", set(), self.NOTE, diff))
        self.assertEqual(check_pr.check_files("fix(M9): retry covers", {"release-change"}, self.NOTE, diff), [])

    def test_rename_is_seen_by_destination(self):
        files = self.NOTE + ["R100\told.md\tdocs/PARITY.md"]
        self.assertTrue(any("docs/PARITY.md" in p for p in check_pr.check_files("fix(M9): retry covers", set(), files, "")))


if __name__ == "__main__":
    unittest.main()
