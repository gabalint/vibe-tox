#!/usr/bin/env python3
"""Write the in-app update manifest and the nightly build index (Python 3.10+, stdlib only).

The package workflow runs this after the signed bundles are built; the application reads the
files from GitHub releases. See docs/development/releases.md#in-app-updates and ADR 0020.

  manifest  The Tauri updater manifest (`latest.json`) of one build: its version, notes,
            publication time and, per updater target, the package URL and Minisign signature.
  index     The nightly index (`nightly.json`): the newest build's manifest plus the retained
            builds, newest first. Prints the tags of builds that fall out of the index.
"""

import argparse
import json
from pathlib import Path
import re
import sys


VERSION = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?"
)
TARGETS = {"windows-x86_64-nsis", "linux-x86_64-appimage", "linux-x86_64-deb", "darwin-aarch64-app"}
TIMESTAMP = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z")
TAG = re.compile(r"nightly-\d{8}\.\d+")
MAX_CHANGES = 50
MAX_CHANGE_CHARS = 200


class FeedError(Exception):
    pass


def check_version(version):
    if not VERSION.fullmatch(version):
        raise FeedError(f"{version} is not a semantic version")
    return version


def check_timestamp(value):
    if not TIMESTAMP.fullmatch(value):
        raise FeedError(f"{value} is not a UTC timestamp such as 2026-10-28T03:00:00Z")
    return value


def change_lines(text):
    lines = [line.strip() for line in text.splitlines()]
    return [line[:MAX_CHANGE_CHARS] for line in lines if line][:MAX_CHANGES]


def manifest(version, published_at, notes, base_url, artifacts, signature_dir=None):
    """`artifacts` maps an updater target to the package path. Its signature is
    `<path>.sig`, or `<signature_dir>/<file name>.sig` when a signature folder is given."""
    if not base_url.startswith("https://") or base_url.endswith("/"):
        raise FeedError("The base URL must be https:// without a trailing slash")
    if not artifacts:
        raise FeedError("A manifest needs at least one package")
    platforms = {}
    for target, package in sorted(artifacts.items()):
        if target not in TARGETS:
            raise FeedError(f"Unknown updater target {target}")
        package = Path(package)
        signature = Path(signature_dir or package.parent) / f"{package.name}.sig"
        if not package.is_file() or package.stat().st_size == 0:
            raise FeedError(f"Package {package} is missing or empty")
        if not signature.is_file() or not signature.read_text().strip():
            raise FeedError(f"Signature {signature} is missing or empty")
        platforms[target] = {
            "signature": signature.read_text().strip(),
            "url": f"{base_url}/{package.name}",
        }
    return {
        "version": check_version(version),
        "notes": "\n".join(change_lines(notes)),
        "pub_date": check_timestamp(published_at),
        "platforms": platforms,
    }


def index(previous, build, newest_manifest, keep):
    """The nightly index with `build` first, then the previous builds; dropped tags second."""
    if keep < 1:
        raise FeedError("Keep at least one build")
    if not TAG.fullmatch(build["tag"]):
        raise FeedError(f"{build['tag']} is not a nightly build tag")
    check_version(build["version"])
    if newest_manifest["version"] != build["version"]:
        raise FeedError("The newest manifest is not the newest build's")
    earlier = [entry for entry in (previous or {}).get("builds", []) if entry.get("tag") != build["tag"]]
    builds = [build, *earlier]
    document = {**newest_manifest, "builds": builds[:keep]}
    dropped = [entry["tag"] for entry in builds[keep:] if TAG.fullmatch(str(entry.get("tag", "")))]
    return document, dropped


def artifact_pairs(values):
    pairs = {}
    for value in values:
        target, separator, path = value.partition("=")
        if not separator or not path:
            raise FeedError(f"{value} is not TARGET=PATH")
        if target in pairs:
            raise FeedError(f"{target} given twice")
        pairs[target] = path
    return pairs


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    write = commands.add_parser("manifest", help="write one build's updater manifest")
    write.add_argument("--version", required=True)
    write.add_argument("--published-at", required=True, help="UTC time, such as 2026-10-28T03:00:00Z")
    write.add_argument("--notes-file", required=True, type=Path)
    write.add_argument("--base-url", required=True, help="release download URL of the packages")
    write.add_argument("--artifact", action="append", default=[], help="TARGET=PATH; PATH.sig is its signature")
    write.add_argument("--signature-dir", type=Path, help="folder of the <file name>.sig signatures")
    write.add_argument("--output", required=True, type=Path)
    merge = commands.add_parser("index", help="write the nightly index and print the tags it drops")
    merge.add_argument("--previous", type=Path, help="the published index, when there is one")
    merge.add_argument("--manifest", required=True, type=Path, help="the newest build's manifest")
    merge.add_argument("--tag", required=True)
    merge.add_argument("--commit", required=True)
    merge.add_argument("--manifest-url", required=True)
    merge.add_argument("--keep", type=int, default=10)
    merge.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        if args.command == "manifest":
            document = manifest(
                args.version,
                args.published_at,
                args.notes_file.read_text(),
                args.base_url,
                artifact_pairs(args.artifact),
                args.signature_dir,
            )
        else:
            newest = json.loads(args.manifest.read_text())
            previous = None
            if args.previous and args.previous.is_file():
                try:
                    previous = json.loads(args.previous.read_text())
                except json.JSONDecodeError:
                    print("warning: the previous nightly index is not JSON; starting a new one", file=sys.stderr)
            if not re.fullmatch(r"[0-9a-f]{40}", args.commit):
                raise FeedError(f"{args.commit} is not a full commit SHA")
            build = {
                "version": newest["version"],
                "tag": args.tag,
                "publishedAt": newest["pub_date"],
                "commit": args.commit,
                "changes": newest["notes"].splitlines(),
                "manifestUrl": args.manifest_url,
            }
            document, dropped = index(previous, build, newest, args.keep)
            for tag in dropped:
                print(tag)
        args.output.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    except FeedError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
