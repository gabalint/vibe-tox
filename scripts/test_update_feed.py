"""Regression tests for the in-app update manifest and the nightly build index."""

from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest

from update_feed import FeedError, former_repositories, index, legacy_manifest, main, manifest


BASE = "https://github.com/example/tox/releases/download/nightly-20261028.20"
SHA = "a" * 40


class FeedTests(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)

    def tearDown(self):
        self.dir.cleanup()

    def package(self, name, signature="c2lnbmF0dXJl"):
        path = self.root / name
        path.write_bytes(b"package")
        if signature is not None:
            Path(f"{path}.sig").write_text(signature + "\n")
        return str(path)

    def test_manifest_names_each_target_package_and_signature(self):
        document = manifest(
            "0.1.2-nightly.20261028.20",
            "2026-10-28T03:00:00Z",
            "Show invalid field names in red\n\n" + "x" * 300,
            BASE,
            {
                "windows-x86_64-nsis": self.package("app_windows-x64-setup.exe"),
                "linux-x86_64-appimage": self.package("app_linux-amd64.AppImage"),
            },
        )
        self.assertEqual(document["version"], "0.1.2-nightly.20261028.20")
        self.assertEqual(document["notes"], "Show invalid field names in red\n" + "x" * 200)
        windows = document["platforms"]["windows-x86_64-nsis"]
        self.assertEqual(windows, {"signature": "c2lnbmF0dXJl", "url": f"{BASE}/app_windows-x64-setup.exe"})

    def test_manifest_reads_signatures_from_a_separate_folder(self):
        signatures = self.root / "signatures"
        signatures.mkdir()
        (signatures / "e.AppImage.sig").write_text("c2ln")
        package = self.package("e.AppImage", None)
        document = manifest("1.0.0", "2026-10-28T03:00:00Z", "", BASE,
                            {"linux-x86_64-appimage": package}, signatures)
        self.assertEqual(document["platforms"]["linux-x86_64-appimage"]["signature"], "c2ln")
        with self.assertRaises(FeedError):
            manifest("1.0.0", "2026-10-28T03:00:00Z", "", BASE, {"linux-x86_64-appimage": package})

    def test_manifest_refuses_unsigned_unknown_or_unsafe_input(self):
        good = {"windows-x86_64-nsis": self.package("a.exe")}
        cases = [
            ("1.0", "2026-10-28T03:00:00Z", BASE, good),
            ("1.0.0", "yesterday", BASE, good),
            ("1.0.0", "2026-10-28T03:00:00Z", "http://github.com/x", good),
            ("1.0.0", "2026-10-28T03:00:00Z", BASE + "/", good),
            ("1.0.0", "2026-10-28T03:00:00Z", BASE, {}),
            ("1.0.0", "2026-10-28T03:00:00Z", BASE, {"windows-x86_64-msi": self.package("b.msi")}),
            ("1.0.0", "2026-10-28T03:00:00Z", BASE, {"linux-x86_64-deb": self.package("c.deb", None)}),
            ("1.0.0", "2026-10-28T03:00:00Z", BASE, {"linux-x86_64-deb": self.package("d.deb", " ")}),
        ]
        for version, published, base, artifacts in cases:
            with self.subTest(version=version, published=published, base=base, artifacts=artifacts):
                with self.assertRaises(FeedError):
                    manifest(version, published, "", base, artifacts)

    def build(self, run, tag=None):
        version = f"0.1.2-nightly.20261028.{run}"
        return {
            "version": version,
            "tag": tag or f"nightly-20261028.{run}",
            "publishedAt": "2026-10-28T03:00:00Z",
            "commit": SHA,
            "changes": [],
            "manifestUrl": f"{BASE}/latest.json",
        }

    def test_index_puts_the_new_build_first_and_drops_the_oldest(self):
        previous = {"builds": [self.build(run) for run in (19, 18, 17)]}
        newest = {"version": "0.1.2-nightly.20261028.20", "platforms": {}}
        document, dropped = index(previous, self.build(20), newest, keep=3)
        self.assertEqual([entry["tag"] for entry in document["builds"]],
                         ["nightly-20261028.20", "nightly-20261028.19", "nightly-20261028.18"])
        self.assertEqual(dropped, ["nightly-20261028.17"])
        self.assertEqual(document["version"], newest["version"])

    def test_index_replaces_a_rebuilt_tag_and_starts_without_a_previous_index(self):
        previous = {"builds": [self.build(20), self.build(19)]}
        newest = {"version": "0.1.2-nightly.20261028.20", "platforms": {}}
        document, dropped = index(previous, self.build(20), newest, keep=10)
        self.assertEqual(len(document["builds"]), 2)
        self.assertEqual(dropped, [])
        document, _ = index(None, self.build(20), newest, keep=10)
        self.assertEqual(len(document["builds"]), 1)

    def test_index_refuses_a_mismatched_or_unsafe_build(self):
        newest = {"version": "0.1.2-nightly.20261028.20", "platforms": {}}
        with self.assertRaises(FeedError):
            index(None, self.build(21), newest, keep=10)
        with self.assertRaises(FeedError):
            index(None, self.build(20, tag="v0.1.2"), newest, keep=10)
        with self.assertRaises(FeedError):
            index(None, self.build(20), newest, keep=0)

    def test_command_line_writes_both_files_and_prints_dropped_tags(self):
        notes = self.root / "notes.txt"
        notes.write_text("One change\n")
        latest = self.root / "latest.json"
        args = [
            "manifest", "--version", "0.1.2-nightly.20261028.20", "--published-at", "2026-10-28T03:00:00Z",
            "--notes-file", str(notes), "--base-url", BASE, "--output", str(latest),
            "--artifact", f"windows-x86_64-nsis={self.package('a.exe')}",
        ]
        self.assertEqual(main(args), 0)
        previous = self.root / "previous.json"
        previous.write_text(json.dumps({"builds": [self.build(19)]}))
        output = self.root / "nightly.json"
        printed = io.StringIO()
        with redirect_stdout(printed):
            code = main([
                "index", "--previous", str(previous), "--manifest", str(latest), "--tag", "nightly-20261028.20",
                "--commit", SHA, "--manifest-url", f"{BASE}/latest.json", "--keep", "1", "--output", str(output),
            ])
        self.assertEqual(code, 0)
        self.assertEqual(printed.getvalue().split(), ["nightly-20261028.19"])
        document = json.loads(output.read_text())
        self.assertEqual(document["builds"][0]["changes"], ["One change"])
        self.assertIn("windows-x86_64-nsis", document["platforms"])
        with redirect_stderr(io.StringIO()):
            self.assertEqual(main(["index", "--manifest", str(latest), "--tag", "nightly-20261028.20",
                                   "--commit", "abc", "--manifest-url", BASE, "--output", str(output)]), 1)

    def renamed_build(self, run, repository):
        build = self.build(run)
        tag = build["tag"]
        return {**build, "manifestUrl": f"https://github.com/{repository}/releases/download/{tag}/latest.json"}

    def test_index_names_the_repository_and_lists_the_newest_build_under_former_names(self):
        previous = {"builds": [
            self.renamed_build(19, "openbga/tox"),
            self.renamed_build(18, "gabalint/vibe-tox"),
        ]}
        current = "openbga/steady-state-lims"
        newest = {"version": "0.1.2-nightly.20261028.20", "platforms": {}, "repository": current}
        build = self.renamed_build(20, current)
        document, dropped = index(previous, build, newest, keep=10, repository=current)
        self.assertEqual(document["repository"], current)
        self.assertEqual(document["formerRepositories"], ["gabalint/vibe-tox", "openbga/tox"])
        urls = [entry["manifestUrl"] for entry in document["builds"] if entry["tag"] == build["tag"]]
        self.assertEqual(urls, [
            build["manifestUrl"],
            "https://github.com/gabalint/vibe-tox/releases/download/nightly-20261028.20/latest-gabalint-vibe-tox.json",
            "https://github.com/openbga/tox/releases/download/nightly-20261028.20/latest-openbga-tox.json",
        ])
        self.assertEqual(dropped, [])
        # The former names are carried even after every build under them has left the index.
        later = {**self.renamed_build(21, current)}
        newer = {**newest, "version": later["version"]}
        document, dropped = index(document, later, newer, keep=1, repository=current)
        self.assertEqual(document["formerRepositories"], ["gabalint/vibe-tox", "openbga/tox"])
        self.assertEqual({entry["tag"] for entry in document["builds"]}, {later["tag"]})
        self.assertEqual(len(document["builds"]), 3)
        self.assertEqual(dropped, ["nightly-20261028.20", "nightly-20261028.19", "nightly-20261028.18"])

    def test_a_former_names_manifest_copy_moves_only_the_package_addresses(self):
        current, former = "openbga/steady-state-lims", "openbga/tox"
        base = f"https://github.com/{current}/releases/download/nightly-20261028.20"
        newest = {"version": "1.0.0", "repository": current,
                  "platforms": {"windows-x86_64-nsis": {"signature": "c2ln", "url": f"{base}/a.exe"}}}
        copy = legacy_manifest(newest, current, former)
        self.assertEqual(copy["platforms"]["windows-x86_64-nsis"], {
            "signature": "c2ln",
            "url": "https://github.com/openbga/tox/releases/download/nightly-20261028.20/a.exe",
        })
        self.assertEqual(copy["version"], "1.0.0")
        foreign = {**newest, "platforms": {"windows-x86_64-nsis": {"signature": "c2ln", "url": BASE + "/a.exe"}}}
        with self.assertRaises(FeedError):
            legacy_manifest(foreign, current, former)

    def test_former_names_ignore_the_current_name_and_other_addresses(self):
        previous = {"formerRepositories": ["a/b"], "builds": [
            {"manifestUrl": "https://github.com/x/y/releases/download/t/latest.json"},
            {"manifestUrl": "https://github.com/cur/rent/releases/download/t/latest.json"},
            {"manifestUrl": "https://example.com/z/w/releases/download/t/latest.json"},
        ]}
        self.assertEqual(former_repositories(previous, "cur/rent"), ["a/b", "x/y"])
        self.assertEqual(former_repositories(None, "cur/rent"), [])
        with self.assertRaises(FeedError):
            index(None, self.build(20), {"version": self.build(20)["version"]}, keep=1, repository="a/b/c")

    def test_command_line_writes_the_former_names_manifest_copies(self):
        current = "openbga/steady-state-lims"
        base = f"https://github.com/{current}/releases/download/nightly-20261028.20"
        notes = self.root / "notes.txt"
        notes.write_text("One change\n")
        latest = self.root / "latest.json"
        self.assertEqual(main([
            "manifest", "--version", "0.1.2-nightly.20261028.20", "--published-at", "2026-10-28T03:00:00Z",
            "--notes-file", str(notes), "--base-url", base, "--repository", current, "--output", str(latest),
            "--artifact", f"windows-x86_64-nsis={self.package('a.exe')}",
        ]), 0)
        self.assertEqual(json.loads(latest.read_text())["repository"], current)
        previous = self.root / "previous.json"
        previous.write_text(json.dumps({"builds": [self.renamed_build(19, "openbga/tox")]}))
        legacy = self.root / "legacy"
        with redirect_stdout(io.StringIO()):
            code = main([
                "index", "--previous", str(previous), "--manifest", str(latest), "--tag", "nightly-20261028.20",
                "--commit", SHA, "--manifest-url", f"{base}/latest.json", "--repository", current,
                "--legacy-dir", str(legacy), "--output", str(self.root / "nightly.json"),
            ])
        self.assertEqual(code, 0)
        copy = json.loads((legacy / "latest-openbga-tox.json").read_text())
        self.assertTrue(copy["platforms"]["windows-x86_64-nsis"]["url"].startswith(
            "https://github.com/openbga/tox/releases/download/nightly-20261028.20/"))


if __name__ == "__main__":
    unittest.main()
