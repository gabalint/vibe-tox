"""Focused checks for Markdown targets and mismatched license declarations."""

import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest

from check_repository_metadata import check_licenses, check_markdown


class MetadataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def write_json(self, name, value):
        self.write(name, json.dumps(value))

    def test_local_links_anchors_references_and_code_examples(self):
        self.write("docs/target.md", "# First\n## Two words\n## Two words\n")
        self.write("README.md", '[link](docs/target.md#two-words-1)\n[remote](https://example.invalid)\n[x]: docs/target.md#first\n```md\n[example](missing.md)\n```\n')
        self.assertEqual(check_markdown(self.root, ["README.md"]), ([], 2))

    def test_missing_targets_and_anchors_fail(self):
        self.write("target.md", "# First\n")
        self.write("README.md", "[broken](gone.md)\n[anchor](target.md#missing)\n")
        errors, count = check_markdown(self.root, ["README.md"])
        self.assertEqual(count, 2)
        self.assertEqual(len(errors), 2)

    def test_changed_heading_checks_links_in_unchanged_documents(self):
        self.write("guide.md", "# Original heading\n")
        self.write("README.md", "[guide](guide.md#original-heading)\n")
        for args in [
            ("init", "-q"), ("add", "."),
            ("-c", "user.name=Verification test", "-c", "user.email=verification@example.invalid",
             "-c", "commit.gpgsign=false", "commit", "-qm", "Fixture"),
        ]:
            subprocess.run(["git", "-C", str(self.root), *args], check=True, capture_output=True)
        self.write("guide.md", "# Renamed heading\n")
        script = Path(__file__).with_name("check_repository_metadata.py").resolve()
        result = subprocess.run([sys.executable, str(script)], cwd=self.root, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("README.md: missing anchor", result.stderr)

    def test_skills_need_name_and_description(self):
        self.write(".ai/example/SKILL.md", "---\nname: example\n---\n# Example\n")
        self.assertEqual(len(check_markdown(self.root, [".ai/example/SKILL.md"])[0]), 1)
        self.write(".ai/example/SKILL.md", "---\nname: example\ndescription: Example\n---\n# Example\n")
        self.assertEqual(check_markdown(self.root, [".ai/example/SKILL.md"])[0], [])

    def license_fixture(self):
        self.write("Cargo.toml", '[workspace]\nmembers=["crates/*"]\n[workspace.package]\nlicense="GPL-3.0-only"\n')
        self.write("crates/example/Cargo.toml", '[package]\nname="example"\nlicense.workspace=true\n')
        self.write_json("package.json", {"license": "GPL-3.0-only"})
        self.write_json("package-lock.json", {"packages": {"": {"license": "GPL-3.0-only"}}})
        self.write_json("src-tauri/tauri.conf.json", {"bundle": {"license": "GPL-3.0-only", "licenseFile": "../LICENSE"}})
        self.write("LICENSE", "Fixture license\n")

    def test_matching_metadata_passes(self):
        self.license_fixture()
        self.assertEqual(check_licenses(self.root), [])

    def test_mismatched_lockfile_root_fails(self):
        self.license_fixture()
        self.write_json("package-lock.json", {"packages": {"": {"license": "MIT"}}})
        self.assertEqual(len(check_licenses(self.root)), 1)

    def test_wrong_tauri_license_path_fails(self):
        self.license_fixture()
        self.write_json("src-tauri/tauri.conf.json", {"bundle": {"license": "GPL-3.0-only", "licenseFile": "LICENSE"}})
        self.assertEqual(len(check_licenses(self.root)), 1)


if __name__ == "__main__":
    unittest.main()
