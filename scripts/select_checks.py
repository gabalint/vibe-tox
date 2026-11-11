#!/usr/bin/env python3
"""Plan checks from changed inputs; never execute builds or tests (Python 3.11+)."""

import argparse
import json
from pathlib import Path
import subprocess
import tomllib


CHECKS = ("docs", "licenses", "website", "tooling", "rust", "ui", "help", "policy")
TOOLING = {
    "scripts/select_checks.py", "scripts/test_select_checks.py",
    "scripts/check_repository_metadata.py", "scripts/test_repository_metadata.py",
    "scripts/release_version.py", "scripts/test_release_version.py",
    "scripts/update_feed.py", "scripts/test_update_feed.py",
    ".github/workflows/ci.yml", ".github/workflows/package.yml", ".gitignore",
}
EMBEDDED = {
    "docs/architecture/backup-and-export-formats.md",
    "scripts/verify_exit_export.py",
}


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE)


def resolve(root, ref):
    return git(root, "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}").decode().strip()


def changed_paths(root, base, head=None):
    args = ["diff", "--name-only", "--no-renames", "-z", base]
    if head:
        args.append(head)
    paths = git(root, *args, "--").split(b"\0")
    if head is None:
        paths += git(root, "ls-files", "--others", "--exclude-standard", "-z").split(b"\0")
    return sorted({path.decode("utf-8", errors="surrogateescape") for path in paths if path})


def read_version(root, path, revision=None):
    if revision:
        try:
            return git(root, "show", f"{revision}:{path}").decode()
        except (subprocess.CalledProcessError, UnicodeDecodeError):
            return None
    try:
        return (root / path).read_text()
    except (OSError, UnicodeDecodeError):
        return None


def same_values(left, right):
    """Preserve types: True and 1 are equal in Python, but not in manifests."""
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(same_values(left[k], right[k]) for k in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(same_values(a, b) for a, b in zip(left, right))
    return left == right


def remove_fields(document, paths):
    for path in paths:
        parent = document
        for key in path[:-1]:
            parent = parent.get(key, {})
            if not isinstance(parent, dict):
                raise ValueError("Expected an object in license metadata")
        parent.pop(path[-1], None)


def license_only(path, before, after):
    if before is None or after is None:
        return False
    if path.endswith("Cargo.toml"):
        parser = tomllib.loads
        fields = [("package", key) for key in ("license", "license-file")]
        fields += [("workspace", "package", key) for key in ("license", "license-file")]
    else:
        parser = json.loads
        fields = {
            "package.json": [("license",)],
            "package-lock.json": [("packages", "", "license")],
            "src-tauri/tauri.conf.json": [("bundle", "license"), ("bundle", "licenseFile")],
        }.get(path, [])
    try:
        old, new = parser(before), parser(after)
        if not isinstance(old, dict) or not isinstance(new, dict):
            return False
        remove_fields(old, fields)
        remove_fields(new, fields)
        return same_values(old, new)
    except (ValueError, TypeError, AttributeError):
        return False


def classify(path, before=None, after=None):
    manifests = {"package.json", "package-lock.json", "src-tauri/tauri.conf.json"}
    if path in manifests or Path(path).name == "Cargo.toml":
        if license_only(path, before, after):
            return {"licenses", "docs"}, "only license metadata or nonsemantic formatting changed"
        return set(CHECKS), "manifest dependency/build change, addition, deletion or parse uncertainty"
    if path == "deny.toml":
        try:
            old, new = tomllib.loads(before or ""), tomllib.loads(after or "")
            old.pop("licenses", None)
            new.pop("licenses", None)
            if before is not None and after is not None and same_values(old, new):
                return {"licenses"}, "only license policy changed"
        except ValueError:
            pass
        return {"policy", "licenses"}, "dependency policy changed"
    if path in EMBEDDED:
        return {"rust", "help", "docs"}, "embedded exit-export content or verifier changed"
    if path.startswith("docs/help/") and path.endswith(".json"):
        return {"help"}, "bundled help content changed"
    if path.startswith("docs/governance/licenses/") or Path(path).name in {"LICENSE", "NOTICE", "COPYING"}:
        return {"licenses", "docs"}, "license or notice changed"
    if path in TOOLING:
        return {"tooling"}, "check selection or CI wiring changed"
    if path == ".impeccable/config.json":
        return {"tooling"}, "design-review tooling configuration changed"
    if path.startswith("website/"):
        return {"website", "docs"}, "standalone presentation changed"
    if path.startswith(".github/ISSUE_TEMPLATE/"):
        return {"docs"}, "issue template changed"
    if path.startswith("ui/src/generated/"):
        return {"rust", "ui", "help"}, "generated IPC contract changed"
    if path.startswith(("crates/", "src-tauri/", "fixtures/")):
        checks = {"rust", "help", "docs"} if path.endswith(".md") else {"rust", "help"}
        return checks, "Rust, shell or runtime/test input changed"
    if path.startswith("ui/") or path in {"vite.config.ts", "tsconfig.json", "knip.json"}:
        checks = {"ui", "docs"} if path.endswith(".md") else {"ui"}
        return checks, "UI source, assets or configuration changed"
    if path.endswith(".md"):
        return {"docs"}, "documentation changed"
    if path in {"rustfmt.toml", "clippy.toml"}:
        return {"rust", "help"}, "Rust lint configuration changed"
    if path == "Cargo.lock" or path == "rust-toolchain.toml" or path.startswith(".cargo/"):
        return set(CHECKS), "shared Rust dependency or build configuration changed"
    return set(CHECKS), "unclassified input; conservative full checks"


def select(root, base="HEAD", head=None, full=False):
    checks = dict.fromkeys(CHECKS, False)
    reasons = {name: [] for name in CHECKS}
    fallback = "explicit full verification" if full else None
    try:
        base = resolve(root, base)
        head = resolve(root, head) if head else None
        paths = changed_paths(root, base, head)
    except subprocess.CalledProcessError:
        paths, base, head = [], None, None
        fallback = "baseline or comparison unavailable; conservative full checks"
    if fallback:
        for name in CHECKS:
            checks[name] = True
            reasons[name].append(fallback)
    else:
        for path in paths:
            before = after = None
            if path.endswith("Cargo.toml") or path in {
                "package.json", "package-lock.json", "src-tauri/tauri.conf.json", "deny.toml",
            }:
                before = read_version(root, path, base)
                after = read_version(root, path, head)
            selected, why = classify(path, before, after)
            for name in selected:
                checks[name] = True
                reasons[name].append(f"{path}: {why}")
    return {
        "checks": checks, "reasons": reasons, "changed_files": paths,
        "base": base, "head": head, "full": all(checks.values()),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="HEAD")
    parser.add_argument("--head", help="Compare committed endpoints; omit for the current worktree")
    parser.add_argument("--full", action="store_true")
    parser.add_argument("--github-output", type=Path)
    parser.add_argument("--github-summary", type=Path)
    args = parser.parse_args()
    root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").decode().strip())
    plan = select(root, args.base, args.head, args.full)
    print(json.dumps(plan, indent=2))
    if args.github_output:
        with args.github_output.open("a") as output:
            for name, enabled in {**plan["checks"], "full": plan["full"]}.items():
                output.write(f"{name}={str(enabled).lower()}\n")
            for name in ("base", "head"):
                output.write(f"{name}={plan[name] or ''}\n")
    if args.github_summary:
        with args.github_summary.open("a") as summary:
            summary.write("## Selected verification\n\n")
            summary.write("```json\n" + json.dumps(plan, indent=2) + "\n```\n")


if __name__ == "__main__":
    main()
