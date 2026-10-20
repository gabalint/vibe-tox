#!/usr/bin/env python3
"""Read, check and set the application version in every manifest (Python 3.11+).

The Cargo workspace version is the version the application records when it runs.
package.json and package-lock.json mirror it. Releases are tagged `v<major.minor.patch>`.
See docs/development/releases.md.
"""

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parent.parent
RELEASE = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)")
VERSION = re.compile(RELEASE.pattern + r"(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?")
NPM_MANIFESTS = ("package.json", "package-lock.json")


class VersionError(Exception):
    pass


def release_tuple(text):
    match = RELEASE.fullmatch(text)
    return tuple(int(part) for part in match.groups()) if match else None


def local_lock_packages(root):
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    return {package["name"]: package["version"] for package in lock["package"] if "source" not in package}


def manifest_versions(root):
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    package = json.loads((root / "package.json").read_text())
    lock = json.loads((root / "package-lock.json").read_text())
    versions = {
        "Cargo.toml": workspace,
        "package.json": package["version"],
        "package-lock.json": lock["version"],
        "package-lock.json packages['']": lock["packages"][""]["version"],
    }
    for name, version in local_lock_packages(root).items():
        versions[f"Cargo.lock {name}"] = version
    return versions


def current(root):
    versions = manifest_versions(root)
    if len(set(versions.values())) != 1:
        detail = ", ".join(f"{place}={version}" for place, version in sorted(versions.items()))
        raise VersionError(f"Manifest versions disagree: {detail}")
    return next(iter(versions.values()))


def released(root):
    tags = subprocess.check_output(["git", "-C", str(root), "tag", "--list", "v*"], text=True).split()
    return sorted(version for tag in tags if (version := release_tuple(tag[1:])))


def release_version(root):
    version = current(root)
    candidate = release_tuple(version)
    if candidate is None:
        raise VersionError(f"{version} is not a release version (major.minor.patch)")
    tags = released(root)
    if candidate in tags:
        raise VersionError(f"v{version} is already released; set a new version before releasing")
    if tags and candidate < tags[-1]:
        raise VersionError(f"{version} is older than the latest release v{'.'.join(map(str, tags[-1]))}")
    return version


def nightly_version(root, date, run):
    if not re.fullmatch(r"[1-9]\d{7}", date) or run < 1:
        raise VersionError("Nightly stamps need a YYYYMMDD date and a positive run number")
    version = current(root)
    base = release_tuple(version)
    if base is None:
        raise VersionError(f"{version} is not a release version (major.minor.patch)")
    tags = released(root)
    if tags and tags[-1] >= base:
        base = (tags[-1][0], tags[-1][1], tags[-1][2] + 1)
    return f"{'.'.join(map(str, base))}-nightly.{date}.{run}"


def set_version(root, version):
    if not VERSION.fullmatch(version):
        raise VersionError(f"{version} is not a semantic version")
    old = current(root)
    cargo = root / "Cargo.toml"
    text, count = re.subn(
        r'(\[workspace\.package\]\n(?:(?!\[)[^\n]*\n)*?version = ")' + re.escape(old) + '"',
        lambda match: match.group(1) + version + '"',
        cargo.read_text(),
    )
    if count != 1:
        raise VersionError("Cargo.toml has no single [workspace.package] version line")
    cargo.write_text(text)
    lock = root / "Cargo.lock"
    text = lock.read_text()
    for name in local_lock_packages(root):
        text, count = re.subn(
            r'(\[\[package\]\]\nname = "' + re.escape(name) + r'"\nversion = ")' + re.escape(old) + '"',
            lambda match: match.group(1) + version + '"',
            text,
        )
        if count != 1:
            raise VersionError(f"Cargo.lock has no single entry for {name} {old}")
    lock.write_text(text)
    for name in NPM_MANIFESTS:
        path = root / name
        document = json.loads(path.read_text())
        document["version"] = version
        if name == "package-lock.json":
            document["packages"][""]["version"] = version
        path.write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    current(root)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("show", help="print the version after checking that every manifest agrees")
    commands.add_parser("release", help="print the version main may release, or fail with the reason")
    nightly = commands.add_parser("nightly", help="print the nightly version for a UTC date and run number")
    nightly.add_argument("--date", required=True, help="UTC date as YYYYMMDD")
    nightly.add_argument("--run", required=True, type=int, help="workflow run number")
    setter = commands.add_parser("set", help="write a version to every manifest")
    setter.add_argument("version")
    args = parser.parse_args(argv)
    try:
        if args.command == "show":
            print(current(args.root))
        elif args.command == "release":
            print(release_version(args.root))
        elif args.command == "nightly":
            print(nightly_version(args.root, args.date, args.run))
        else:
            set_version(args.root, args.version)
            print(args.version)
    except VersionError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
