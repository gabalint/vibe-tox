#!/usr/bin/env python3
"""Check selected local Markdown links and project license metadata without building."""

import argparse
import json
from pathlib import Path
import re
import unicodedata
from urllib.parse import unquote, urlsplit
import tomllib

from select_checks import git, select


def markdown_anchors(text):
    anchors = set(re.findall(r'(?:id|name)=["\x27]([^"\x27]+)', text))
    seen = {}
    for title in re.findall(r"^#{1,6}\s+(.+?)\s*#*$", text, re.M):
        title = re.sub(r"\[([^]]*)\]\([^)]*\)", r"\1", title)
        title = re.sub(r"<[^>]+>", "", title).lower()
        slug = "".join(c for c in title if c in " -_" or unicodedata.category(c)[0] in "LN")
        slug = slug.replace(" ", "-")
        number = seen.get(slug, 0)
        seen[slug] = number + 1
        anchors.add(slug + (f"-{number}" if number else ""))
    return anchors


def check_markdown(root, paths):
    errors, count = [], 0
    for name in paths:
        path = root / name
        if path.suffix != ".md" or not path.is_file():
            continue
        text = path.read_text()
        if name.endswith("/SKILL.md"):
            frontmatter = text.split("---", 2)
            if len(frontmatter) < 3 or frontmatter[0].strip() or any(
                not re.search(rf"^{key}:\s*\S", frontmatter[1], re.M) for key in ("name", "description")
            ):
                errors.append(f"{name}: missing skill name/description frontmatter")
        # Ignore examples inside fenced code blocks, then inspect inline and reference links.
        text = re.sub(r"^(`{3,}|~{3,}).*?^\1\s*$", "", text, flags=re.M | re.S)
        links = re.findall(r"!?\[[^\]]*\]\(([^)]+)\)", text)
        links += re.findall(r"^\s*\[[^]]+\]:\s*(\S+)", text, re.M)
        for target in links:
            target = target.split(' "', 1)[0].strip("<>")
            url = urlsplit(target)
            if url.scheme or url.netloc:
                continue
            destination = (path.parent / unquote(url.path)).resolve() if url.path else path
            count += 1
            if not destination.exists():
                errors.append(f"{name}: missing target {target}")
            elif url.fragment and destination.suffix == ".md":
                if unquote(url.fragment) not in markdown_anchors(destination.read_text()):
                    errors.append(f"{name}: missing anchor {target}")
    return errors, count


def check_licenses(root):
    errors = []
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    expected = cargo["workspace"]["package"]["license"]
    package = json.loads((root / "package.json").read_text())
    lock = json.loads((root / "package-lock.json").read_text())
    tauri = json.loads((root / "src-tauri/tauri.conf.json").read_text())["bundle"]
    for name, actual in {
        "package.json": package.get("license"),
        "package-lock.json root": lock["packages"][""].get("license"),
        "Tauri bundle": tauri.get("license"),
    }.items():
        if actual != expected:
            errors.append(f"{name}: license {actual!r} differs from workspace {expected!r}")
    for pattern in cargo["workspace"]["members"]:
        members = list(root.glob(pattern))
        if not members:
            errors.append(f"Workspace member {pattern!r} has no match")
        for member in members:
            declared = tomllib.loads((member / "Cargo.toml").read_text())["package"].get("license")
            if declared != expected and declared != {"workspace": True}:
                errors.append(f"{member.relative_to(root)}: license is not {expected!r} or inherited")
    license_file = tauri.get("licenseFile")
    if not license_file or (root / "src-tauri" / license_file).resolve() != (root / "LICENSE").resolve():
        errors.append("Tauri bundle.licenseFile must point to the project LICENSE")
    if not (root / "LICENSE").is_file():
        errors.append("Project LICENSE is missing")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="HEAD")
    parser.add_argument("--head")
    parser.add_argument("--full", action="store_true")
    args = parser.parse_args()
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").decode().strip())
    if args.head and git(root, "rev-parse", "HEAD").decode().strip() != git(
        root, "rev-parse", "--verify", "--end-of-options", f"{args.head}^{{commit}}"
    ).decode().strip():
        parser.error("metadata checks require the compared head to be checked out")
    plan = select(root, args.base, args.head, args.full)
    errors = []
    if plan["checks"]["docs"]:
        # A changed heading or removed file can break links in unchanged documents.
        tracked = git(root, "ls-files", "-z").decode().split("\0")
        paths = sorted(set(tracked + plan["changed_files"]))
        failures, count = check_markdown(root, paths)
        errors.extend(failures)
        print(f"Checked {count} local Markdown links (including anchors).")
    if plan["checks"]["licenses"]:
        errors.extend(check_licenses(root))
        print("Checked workspace, npm, lockfile and Tauri license metadata.")
    if errors:
        raise SystemExit("\n".join(errors))
    print("Selected repository metadata checks passed.")


if __name__ == "__main__":
    main()
