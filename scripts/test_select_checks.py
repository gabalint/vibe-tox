"""Regression tests for skipped checks, conservative fallbacks and real Git diffs."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from select_checks import CHECKS, classify, select


class ClassificationTests(unittest.TestCase):
    def test_ordinary_changes_select_only_affected_subsystems(self):
        cases = {
            "README.md": {"docs"},
            ".ai/skills/quality-gate/SKILL.md": {"docs"},
            "docs/help/README.md": {"docs"},
            "docs/help/orders.json": {"help"},
            "docs/architecture/backup-and-export-formats.md": {"rust", "help", "docs"},
            "scripts/verify_exit_export.py": {"rust", "help", "docs"},
            "website/index.html": {"website", "docs"},
            "LICENSE": {"licenses", "docs"},
            "docs/governance/licenses/third-party-license.txt": {"licenses", "docs"},
            "crates/lis-orders/src/lib.rs": {"rust", "help"},
            "crates/lis-orders/migrations/0007_change.sql": {"rust", "help"},
            "crates/lis-catalog/standard/catalog-v1.json": {"rust", "help"},
            "fixtures/files/data.csv": {"rust", "help"},
            "src-tauri/capabilities/orders.json": {"rust", "help"},
            "ui/src/features/orders/Orders.tsx": {"ui"},
            "ui/src/content.md": {"ui", "docs"},
            "crates/example/template.md": {"rust", "help", "docs"},
            "ui/src/generated/Order.ts": {"rust", "ui", "help"},
            ".github/workflows/ci.yml": {"tooling"},
            ".github/ISSUE_TEMPLATE/bug-report.yml": {"docs"},
            "scripts/select_checks.py": {"tooling"},
            "Cargo.lock": set(CHECKS),
            "rust-toolchain.toml": set(CHECKS),
            "unexpected/runtime.data": set(CHECKS),
        }
        for path, expected in cases.items():
            with self.subTest(path=path):
                self.assertEqual(classify(path)[0], expected)

    def test_license_only_manifests_do_not_build_or_test_application(self):
        cases = [
            ("Cargo.toml", '[workspace.package]\nlicense="MIT"\n', '[workspace.package]\nlicense="GPL-3.0-only"\n'),
            ("crates/example/Cargo.toml", '[package]\nlicense="MIT"\n', '[package]\nlicense.workspace=true\n'),
            ("package.json", '{"license":"MIT","scripts":{"build":"vite"}}', '{"license":"GPL-3.0-only","scripts":{"build":"vite"}}'),
            ("package-lock.json", '{"packages":{"":{"license":"MIT"},"node_modules/x":{"license":"MIT"}}}', '{"packages":{"":{"license":"GPL-3.0-only"},"node_modules/x":{"license":"MIT"}}}'),
            ("src-tauri/tauri.conf.json", '{"bundle":{"license":"MIT","active":true}}', '{"bundle":{"license":"GPL-3.0-only","licenseFile":"../LICENSE","active":true}}'),
        ]
        for path, old, new in cases:
            with self.subTest(path=path):
                self.assertEqual(classify(path, old, new)[0], {"docs", "licenses"})

    def test_mixed_license_and_behavior_changes_select_full_checks(self):
        cases = [
            ("Cargo.toml", '[workspace.package]\nlicense="MIT"\n[workspace.dependencies]\na="1"', '[workspace.package]\nlicense="GPL-3.0-only"\n[workspace.dependencies]\na="2"'),
            ("package.json", '{"license":"MIT","scripts":{"build":"vite"}}', '{"license":"GPL-3.0-only","scripts":{"build":"different"}}'),
            ("package-lock.json", '{"packages":{"":{"license":"MIT"},"node_modules/x":{"license":"MIT"}}}', '{"packages":{"":{"license":"GPL-3.0-only"},"node_modules/x":{"license":"GPL-3.0-only"}}}'),
            ("src-tauri/tauri.conf.json", '{"bundle":{"license":"MIT","active":true}}', '{"bundle":{"license":"GPL-3.0-only","active":1}}'),
        ]
        for path, old, new in cases:
            with self.subTest(path=path):
                self.assertEqual(classify(path, old, new)[0], set(CHECKS))

    def test_invalid_added_or_deleted_manifest_is_not_license_only(self):
        for old, new in [(None, '{}'), ('{}', None), ('{}', 'broken'), ('{}', '[]')]:
            with self.subTest(old=old, new=new):
                self.assertEqual(classify("package.json", old, new)[0], set(CHECKS))

    def test_license_policy_does_not_trigger_advisory_or_build_checks(self):
        old = '[licenses]\nallow=["MIT"]\n[bans]\nmultiple-versions="warn"'
        new = old.replace('["MIT"]', '["MIT", "GPL-3.0-only"]')
        self.assertEqual(classify("deny.toml", old, new)[0], {"licenses"})
        self.assertEqual(classify("deny.toml", old, new.replace('"warn"', '"deny"'))[0], {"licenses", "policy"})


class GitSelectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Verification test")
        self.git("config", "user.email", "verification@example.invalid")
        self.write("README.md", "# Example\n")
        self.write("package.json", '{"license":"MIT"}\n')
        self.write("crates/example/src/lib.rs", "pub fn example() {}\n")
        self.commit()
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.root), *args], stderr=subprocess.PIPE).decode()

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self):
        # These index/history writes are confined to the disposable fixture repository.
        self.git("add", ".")
        self.git("-c", "commit.gpgsign=false", "commit", "-qm", "Fixture")

    def selected(self, **kwargs):
        return {name for name, enabled in select(self.root, **kwargs)["checks"].items() if enabled}

    def test_no_change_selects_nothing(self):
        self.assertEqual(self.selected(), set())

    def test_staged_unstaged_and_untracked_are_combined(self):
        self.write("README.md", "# Changed\n")
        self.git("add", "README.md")
        self.write("package.json", '{"license":"GPL-3.0-only"}\n')
        self.write("website/new file.html", "hello\n")
        self.assertEqual(self.selected(), {"docs", "licenses", "website"})

    def test_renamed_or_deleted_code_cannot_hide_behind_markdown(self):
        (self.root / "crates/example/src/lib.rs").rename(self.root / "notes.md")
        self.git("add", "--all")
        plan = select(self.root)
        self.assertIn("crates/example/src/lib.rs", plan["changed_files"])
        self.assertIn("notes.md", plan["changed_files"])
        self.assertEqual(self.selected(), {"rust", "help", "docs"})

    def test_commit_comparison_ignores_worktree_and_compares_all_commits(self):
        self.write("package.json", '{"license":"GPL-3.0-only"}\n')
        self.commit()
        self.write("README.md", "# Changed\n")
        self.commit()
        self.write("ui/src/example.ts", "export const value = 1;\n")
        self.assertEqual(self.selected(base=self.base, head="HEAD"), {"licenses", "docs"})
        self.assertEqual(self.selected(base=self.base), {"licenses", "docs", "ui"})

    def test_missing_baseline_and_full_override_select_everything(self):
        self.assertEqual(self.selected(base="0" * 40, head="HEAD"), set(CHECKS))
        self.assertEqual(self.selected(full=True), set(CHECKS))

    def test_cli_outputs_explicit_flags_for_ci(self):
        self.write("README.md", "# Changed\n")
        with tempfile.TemporaryDirectory() as output_dir:
            output = Path(output_dir) / "output"
            script = Path(__file__).with_name("select_checks.py").resolve()
            result = subprocess.check_output([
                sys.executable, str(script), "--github-output", str(output),
            ], cwd=self.root)
            self.assertTrue(json.loads(result)["checks"]["docs"])
            fields = dict(line.split("=", 1) for line in output.read_text().splitlines())
            self.assertEqual(fields["rust"], "false")
            self.assertEqual(fields["docs"], "true")
            self.assertEqual(fields["base"], self.base)


if __name__ == "__main__":
    unittest.main()
