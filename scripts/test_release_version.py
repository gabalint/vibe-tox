"""Regression tests for manifest version agreement, release refusal and nightly stamps."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from release_version import VersionError, current, main, nightly_version, release_version, set_version


CARGO_TOML = """[workspace]
members = ["crates/a"]

[workspace.package]
edition = "2024"
version = "0.1.0"

[workspace.dependencies]
serde = { version = "0.1.0" }
"""
CARGO_LOCK = """version = 4

[[package]]
name = "a"
version = "0.1.0"
dependencies = [
 "serde",
]

[[package]]
name = "serde"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"""


class VersionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text(CARGO_TOML)
        (self.root / "Cargo.lock").write_text(CARGO_LOCK)
        (self.root / "package.json").write_text(json.dumps({"name": "x", "version": "0.1.0"}, indent=2) + "\n")
        lock = {"name": "x", "version": "0.1.0", "packages": {"": {"name": "x", "version": "0.1.0"}}}
        (self.root / "package-lock.json").write_text(json.dumps(lock, indent=2) + "\n")
        self.git("init", "-q")
        self.git("-c", "user.name=t", "-c", "user.email=t@example.invalid", "commit", "-q", "--allow-empty", "-m", "c")

    def tearDown(self):
        self.temp.cleanup()

    def git(self, *args):
        subprocess.run(["git", "-C", str(self.root), *args], check=True)

    def test_set_updates_only_local_versions(self):
        set_version(self.root, "0.2.0")
        self.assertEqual(current(self.root), "0.2.0")
        self.assertIn('serde = { version = "0.1.0" }', (self.root / "Cargo.toml").read_text())
        self.assertIn('name = "serde"\nversion = "0.1.0"', (self.root / "Cargo.lock").read_text())

    def test_disagreeing_manifests_are_refused(self):
        (self.root / "package.json").write_text(json.dumps({"name": "x", "version": "0.2.0"}))
        with self.assertRaisesRegex(VersionError, "disagree"):
            current(self.root)

    def test_release_requires_a_new_plain_version(self):
        self.assertEqual(release_version(self.root), "0.1.0")
        self.git("tag", "v0.1.0")
        with self.assertRaisesRegex(VersionError, "already released"):
            release_version(self.root)
        self.git("tag", "v0.3.0")
        set_version(self.root, "0.2.0")
        with self.assertRaisesRegex(VersionError, "older than the latest release v0.3.0"):
            release_version(self.root)
        set_version(self.root, "0.4.0-rc.1")
        with self.assertRaisesRegex(VersionError, "not a release version"):
            release_version(self.root)

    def test_nightly_sorts_after_the_latest_release(self):
        self.assertEqual(nightly_version(self.root, "20261021", 7), "0.1.0-nightly.20261021.7")
        self.git("tag", "v0.1.0")
        self.git("tag", "nightly")
        self.assertEqual(nightly_version(self.root, "20261021", 7), "0.1.1-nightly.20261021.7")
        set_version(self.root, "0.2.0")
        self.assertEqual(nightly_version(self.root, "20261021", 7), "0.2.0-nightly.20261021.7")
        with self.assertRaises(VersionError):
            nightly_version(self.root, "2026-10-21", 7)

    def test_cli_reports_errors_without_a_traceback(self):
        output, errors = io.StringIO(), io.StringIO()
        with redirect_stdout(output), redirect_stderr(errors):
            self.assertEqual(main(["--root", str(self.root), "set", "not-a-version"]), 1)
            self.assertEqual(main(["--root", str(self.root), "show"]), 0)
        self.assertEqual(output.getvalue(), "0.1.0\n")
        self.assertIn("not a semantic version", errors.getvalue())


if __name__ == "__main__":
    unittest.main()
