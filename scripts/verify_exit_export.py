#!/usr/bin/env python3
"""Independent verifier for a Vibe Toxicology LIS exit package (vibe-tox-exit-1).

Written from docs/architecture/backup-and-export-formats.md alone (NFR-12, REQ-OPS-010,
ADR 0008), with the Python standard library only. It checks, in the document's order:
the manifest, every listed file's SHA-256 and length, the database's integrity, installation
and migration ledger, the migration sources, each audit chain recomputed as `sha256-v1`,
the anchor lines against the database and `audit_seals`, and the artifact references.

Usage: python3 verify_exit_export.py <package folder>
Exit code 0 only when everything verifies; 1 when a check fails; 2 on wrong usage.
The report names files, tables, streams, sequence numbers and digests, never row content.
"""

import hashlib
import json
import os
import pathlib
import re
import sqlite3
import sys

GENESIS = "0" * 64
STREAMS = ("clinical", "security", "access")
HASH_ALG = "sha256-v1"
MANIFEST_KEYS = {
    "format", "installationId", "exportId", "createdAt", "appVersion", "schema", "files",
    "heads", "sealCount", "artifactReferences", "artifactProblems",
}
REQUIRED_FILES = {
    "README.txt", "database/lis.sqlite3", "audit/anchors.jsonl", "schema/schema.sql",
    "schema/data-dictionary.json", "schema/data-dictionary.md", "tools/verify_export.py",
}
ANCHOR_KEYS = {
    "format", "installation", "stream", "seal", "firstSeq", "lastSeq", "prevHash",
    "headHash", "sealedAt", "hashAlg",
}
# The 20 hashed columns of `audit_events`, in hashing order.
EVENT_COLUMNS = (
    "hash_alg", "stream", "seq", "prev_hash", "occurred_at", "recorded_at", "principal_kind",
    "principal_id", "session_id", "workstation_id", "action", "outcome", "record_type",
    "record_id", "old_revision", "new_revision", "reason", "authority", "correlation_id",
    "details",
)
# The columns of `audit_seals`, as the format document lists them ("Seals in the database").
SEAL_COLUMNS = (
    "stream", "seal_no", "first_seq", "last_seq", "prev_hash", "head_hash", "sealed_at",
    "hash_alg",
)
HEX64 = re.compile(r"^[0-9a-f]{64}$")
TIME = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z$")
UUID7 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
TEST_VECTOR = "ac1f0df2609c488cc06475e88feac1c1ab293969768f2206bd669dfc1cf8d55a"


class Report:
    """Collects failures; prints each check as it completes."""

    def __init__(self):
        self.problems = 0

    def ok(self, text):
        print(f"OK    {text}")

    def fail(self, text):
        self.problems += 1
        print(f"FAIL  {text}")

    def check(self, condition, text):
        if condition:
            self.ok(text)
        else:
            self.fail(text)
        return condition


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def encode_value(value):
    """One `sha256-v1` field: 0x00 for NULL, else 0x01, 4-byte big-endian length, UTF-8."""
    if value is None:
        return b"\x00"
    if isinstance(value, bool) or not isinstance(value, (int, str)):
        raise ValueError("an audit column holds neither text nor an integer")
    data = str(value).encode("utf-8")
    return b"\x01" + len(data).to_bytes(4, "big") + data


def event_hash(row):
    return hashlib.sha256(b"".join(encode_value(value) for value in row)).hexdigest()


def hash_matches_test_vector():
    ids = "01900000-0000-7000-8000-00000000000"
    time = "2026-10-12T10:00:00.000000000Z"
    row = (
        HASH_ALG, "clinical", 1, GENESIS, time, time, "user", ids + "1", ids + "2", ids + "3",
        "specimen.receive", "succeeded", "specimen", ids + "4", None, 1, None, "accession",
        ids + "5", "{}",
    )
    return event_hash(row) == TEST_VECTOR


def reject_duplicates(pairs):
    keys = [key for key, _ in pairs]
    if len(keys) != len(set(keys)):
        raise ValueError("a key is repeated")
    return dict(pairs)


def load_json(path):
    with open(path, encoding="utf-8") as file:
        return json.load(file, object_pairs_hook=reject_duplicates)


def safe_relative(path):
    """A manifest path must stay inside the package."""
    if not isinstance(path, str) or not path or "\\" in path or path.startswith("/"):
        return False
    return all(part not in ("", ".", "..") for part in path.split("/"))


def package_files(root):
    found = set()
    for folder, _, names in os.walk(root):
        for name in names:
            relative = os.path.relpath(os.path.join(folder, name), root)
            found.add(relative.replace(os.sep, "/"))
    return found


# 1. Manifest and 2. files ---------------------------------------------------------------


def check_manifest(root, report):
    try:
        manifest = load_json(os.path.join(root, "manifest.json"))
    except (OSError, ValueError) as error:
        report.fail(f"manifest.json is missing or not one JSON object ({type(error).__name__})")
        return None
    if not isinstance(manifest, dict) or set(manifest) != MANIFEST_KEYS:
        report.fail("manifest.json does not hold exactly the documented keys")
        return None
    if not report.check(manifest["format"] == "vibe-tox-exit-1", "manifest format is vibe-tox-exit-1"):
        return None
    if not isinstance(manifest["files"], list) or not all(
        isinstance(entry, dict) and set(entry) == {"path", "sha256", "bytes"}
        and safe_relative(entry["path"]) for entry in manifest["files"]
    ):
        report.fail("manifest files are not [{path, sha256, bytes}] inside the package")
        return None
    return manifest


def check_files(root, manifest, report):
    listed = {entry["path"]: entry for entry in manifest["files"]}
    report.check(len(listed) == len(manifest["files"]), "no file is listed twice")
    missing_required = REQUIRED_FILES - set(listed)
    report.check(
        not missing_required,
        f"all fixed required files are listed ({len(missing_required)} missing)",
    )
    extra = package_files(root) - set(listed) - {"manifest.json"}
    report.check(not extra, f"no unlisted file in the package ({len(extra)} found)")
    bad = 0
    for path, entry in sorted(listed.items()):
        full = os.path.join(root, *path.split("/"))
        if not os.path.isfile(full):
            report.fail(f"listed file is missing: {path}")
            bad += 1
        elif os.path.getsize(full) != entry["bytes"] or sha256_file(full) != entry["sha256"]:
            report.fail(f"listed file has another digest or length: {path}")
            bad += 1
        elif path.startswith("artifacts/") and path[len("artifacts/"):] != entry["sha256"]:
            report.fail(f"artifact name is not the SHA-256 of its bytes: {path}")
            bad += 1
    if not bad:
        report.ok(f"{len(listed)} listed files match their SHA-256 and length")
    return listed


# 3. Database and 4. migrations ----------------------------------------------------------


def open_database(root, report):
    path = pathlib.Path(root, "database", "lis.sqlite3").resolve()
    if not path.is_file():
        report.fail("database/lis.sqlite3 is missing")
        return None
    # Read-only and immutable: verification never writes into the package.
    conn = sqlite3.connect(path.as_uri() + "?mode=ro&immutable=1", uri=True)
    try:
        rows = conn.execute("PRAGMA integrity_check").fetchall()
    except sqlite3.DatabaseError as error:
        report.fail(f"database does not open as SQLite ({type(error).__name__})")
        return None
    if not report.check(rows == [("ok",)], "database integrity_check returns ok"):
        return None
    return conn


def check_installation(conn, manifest, report):
    rows = conn.execute("SELECT installation_id FROM ops_installation").fetchall()
    report.check(
        rows == [(manifest["installationId"],)] and UUID7.match(manifest["installationId"] or ""),
        "ops_installation holds the manifest's installationId",
    )


def check_migrations(root, conn, manifest, report):
    rows = conn.execute(
        "SELECT module, version, name, checksum FROM store_migrations ORDER BY module, version"
    ).fetchall()
    ledger = [{"module": m, "version": v, "name": n, "checksum": c} for m, v, n, c in rows]
    schema = manifest["schema"] if isinstance(manifest["schema"], list) else []
    ordered = sorted(schema, key=lambda e: (str(e.get("module")), str(e.get("version"))))
    report.check(
        sorted(ledger, key=lambda e: (e["module"], str(e["version"]))) == ordered,
        f"store_migrations equals the manifest schema ({len(ledger)} migrations)",
    )
    bad = 0
    for entry in ledger:
        relative = f"schema/migrations/{entry['module']}/{entry['version']:04d}_{entry['name']}.sql"
        full = os.path.join(root, *relative.split("/"))
        if not safe_relative(relative) or not os.path.isfile(full):
            report.fail(f"migration source is missing: {relative}")
            bad += 1
        elif sha256_file(full) != entry["checksum"]:
            report.fail(f"migration source does not match its checksum: {relative}")
            bad += 1
    if not bad:
        report.ok("every migration source matches its checksum")


def check_schema_files(root, conn, manifest, report):
    """The schema SQL and both dictionaries describe this database, not another one."""
    expected_schema = ";\n\n".join(
        row[0] for row in conn.execute(
            "SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY rowid"
        )
    ) + ";\n\n"
    try:
        schema = pathlib.Path(root, "schema", "schema.sql").read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        schema = None
    report.check(schema == expected_schema, "schema/schema.sql equals the database's sqlite_schema")

    try:
        dictionary = load_json(os.path.join(root, "schema", "data-dictionary.json"))
    except (OSError, ValueError):
        dictionary = None
    if not isinstance(dictionary, dict) or set(dictionary) != {"format", "tables"}:
        report.fail("data-dictionary.json is not the documented dictionary object")
        return
    if dictionary["format"] != "vibe-tox-dictionary-1" or not isinstance(dictionary["tables"], list):
        report.fail("data-dictionary.json has another format or no table list")
        return

    references = {
        (entry.get("table"), entry.get("column"))
        for entry in manifest["artifactReferences"] if isinstance(entry, dict)
    }
    expected_objects = conn.execute(
        "SELECT name, type, sql FROM sqlite_schema WHERE type IN ('table', 'view') "
        "AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).fetchall()
    entries = dictionary["tables"]
    names = [entry.get("name") for entry in entries if isinstance(entry, dict)]
    report.check(names == [row[0] for row in expected_objects], "dictionary lists every table and view in name order")
    bad = 0
    for entry, (name, kind, ddl) in zip(entries, expected_objects):
        expected_keys = {
            "name", "kind", "module", "columns", "foreignKeys", "ddl", "indexes", "triggers",
        }
        if not isinstance(entry, dict) or set(entry) != expected_keys:
            bad += 1
            continue
        columns = [
            {
                "name": column,
                "type": type_name,
                "notNull": bool(not_null),
                "default": default,
                "primaryKey": primary_key,
                "generated": hidden >= 2,
                "artifact": (name, column) in references,
            }
            for column, type_name, not_null, default, primary_key, hidden in conn.execute(
                'SELECT name, type, "notnull", dflt_value, pk, hidden '
                "FROM pragma_table_xinfo(?) ORDER BY cid", (name,)
            )
        ]
        foreign_keys = [
            {"from": source, "table": table, "to": target}
            for source, table, target in conn.execute(
                'SELECT "from", "table", "to" FROM pragma_foreign_key_list(?) ORDER BY id, seq',
                (name,),
            )
        ]
        indexes = [row[0] for row in conn.execute(
            "SELECT sql FROM sqlite_schema WHERE type = 'index' AND tbl_name = ? "
            "AND sql IS NOT NULL ORDER BY name", (name,)
        )]
        triggers = [
            {"name": trigger, "ddl": trigger_ddl}
            for trigger, trigger_ddl in conn.execute(
                "SELECT name, sql FROM sqlite_schema WHERE type = 'trigger' AND tbl_name = ? "
                "AND sql IS NOT NULL ORDER BY name", (name,)
            )
        ]
        if (
            entry["name"] != name or entry["kind"] != kind or entry["ddl"] != ddl
            or entry["columns"] != columns or entry["foreignKeys"] != foreign_keys
            or entry["indexes"] != indexes or entry["triggers"] != triggers
        ):
            bad += 1
    report.check(not bad and len(entries) == len(expected_objects), f"JSON dictionary matches database metadata ({bad} bad objects)")

    try:
        markdown = pathlib.Path(root, "schema", "data-dictionary.md").read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        markdown = ""
    missing_markdown = [
        name for name, _, ddl in expected_objects
        if f"## `{name}`" not in markdown or f"```sql\n{ddl}\n```" not in markdown
    ]
    report.check(
        markdown.startswith("# Data dictionary\n") and not missing_markdown,
        f"Markdown dictionary names and gives DDL for every object ({len(missing_markdown)} missing)",
    )


# 5. Audit chains ------------------------------------------------------------------------


def recompute_stream(conn, stream):
    """Returns ({seq: stored hash}, lowest problem seq or None, (head seq, head hash))."""
    columns = ", ".join(EVENT_COLUMNS)
    rows = conn.execute(
        f"SELECT {columns}, hash FROM audit_events WHERE stream = ? ORDER BY seq", (stream,)
    )
    stored, problem, expected, previous = {}, None, 1, GENESIS
    for row in rows:
        seq, prev_hash, hash_value = row[2], row[3], row[-1]
        broken = (
            seq != expected or row[0] != HASH_ALG or prev_hash != previous
            or event_hash(row[:-1]) != hash_value
        )
        if broken and problem is None:
            problem = min(expected, seq) if isinstance(seq, int) else expected
        stored[seq] = hash_value
        expected, previous = (seq + 1 if isinstance(seq, int) else expected + 1), hash_value
    head = (expected - 1, previous) if stored else (0, GENESIS)
    return stored, problem, head


def check_chains(conn, manifest, report):
    known = {row[0] for row in conn.execute("SELECT DISTINCT stream FROM audit_events")}
    report.check(known <= set(STREAMS), "audit_events holds only the three documented streams")
    chains = {stream: recompute_stream(conn, stream) for stream in STREAMS}
    heads = {}
    if isinstance(manifest["heads"], list):
        for entry in manifest["heads"]:
            if isinstance(entry, dict) and set(entry) == {"stream", "seq", "hash"}:
                heads[entry["stream"]] = (entry["seq"], entry["hash"])
    for stream, (stored, problem, head) in chains.items():
        report.check(
            problem is None,
            f"{stream} chain recomputes ({len(stored)} events"
            + ("" if problem is None else f"; first problem at seq {problem}") + ")",
        )
        report.check(heads.get(stream) == head, f"{stream} head equals the manifest (seq {head[0]})")
    report.check(len(heads) == len(manifest["heads"]) == len(STREAMS), "manifest lists one head per stream")
    return chains


# 6. Seals and anchors -------------------------------------------------------------------


def parse_anchor(line):
    """A strictly valid anchor line as a dict; raises ValueError otherwise."""
    anchor = json.loads(line, object_pairs_hook=reject_duplicates)
    if not isinstance(anchor, dict) or set(anchor) != ANCHOR_KEYS:
        raise ValueError("not exactly the anchor keys")
    if anchor["format"] != "vibe-tox-anchor-1" or anchor["hashAlg"] != HASH_ALG:
        raise ValueError("another format or hash algorithm")
    if anchor["stream"] not in STREAMS:
        raise ValueError("unknown stream")
    numbers = (anchor["seal"], anchor["firstSeq"], anchor["lastSeq"])
    if not all(isinstance(n, int) and not isinstance(n, bool) for n in numbers):
        raise ValueError("a number is not an integer")
    if anchor["seal"] < 1 or anchor["firstSeq"] < 1 or anchor["lastSeq"] < anchor["firstSeq"]:
        raise ValueError("a number is out of range")
    hashes = (anchor["prevHash"], anchor["headHash"])
    if not all(isinstance(h, str) and HEX64.match(h) for h in hashes):
        raise ValueError("a hash is not 64 lowercase hexadecimal digits")
    if not isinstance(anchor["sealedAt"], str) or not TIME.match(anchor["sealedAt"]):
        raise ValueError("sealedAt is not RFC 3339 UTC with nine fractional digits")
    if not isinstance(anchor["installation"], str) or not UUID7.match(anchor["installation"]):
        raise ValueError("installation is not a lowercase version 7 UUID")
    return anchor


def read_anchor_lines(root, report):
    path = os.path.join(root, "audit", "anchors.jsonl")
    try:
        with open(path, "rb") as file:
            data = file.read().decode("utf-8")
    except (OSError, UnicodeDecodeError):
        report.fail("audit/anchors.jsonl is missing or not UTF-8")
        return None
    if data and not data.endswith("\n"):
        report.fail("audit/anchors.jsonl does not end each line with a line feed")
    anchors, invalid = [], 0
    for number, line in enumerate(data.splitlines(), start=1):
        try:
            anchors.append(parse_anchor(line))
        except ValueError as error:
            report.fail(f"anchor line {number} is invalid: {error}")
            invalid += 1
    return anchors if not invalid else None


def anchor_status(anchor, chains):
    stored, problem, _ = chains[anchor["stream"]]
    first, last = anchor["firstSeq"], anchor["lastSeq"]
    if last not in stored:
        return "local_events_missing"
    prev_ok = (anchor["prevHash"] == GENESIS) if first == 1 else stored.get(first - 1) == anchor["prevHash"]
    if stored[last] == anchor["headHash"] and prev_ok and (problem is None or problem > last):
        return "matched"
    return "anchor_mismatch"


def local_seals(conn, report):
    columns = {row[1] for row in conn.execute("PRAGMA table_info(audit_seals)")}
    if not set(SEAL_COLUMNS) <= columns:
        report.fail("audit_seals does not have the expected columns")
        return None
    rows = conn.execute(
        f"SELECT {', '.join(SEAL_COLUMNS)} FROM audit_seals ORDER BY stream, seal_no"
    ).fetchall()
    return [
        {
            "stream": s, "seal": n, "firstSeq": f, "lastSeq": l, "prevHash": p,
            "headHash": h, "sealedAt": t, "hashAlg": a,
        }
        for s, n, f, l, p, h, t, a in rows
    ]


def check_seal_chain(seals, chains, report):
    """Each local seal continues the previous one and names the stored hash of its event."""
    bad, previous = 0, {}
    for seal in seals:
        stored = chains[seal["stream"]][0] if seal["stream"] in chains else {}
        number, last_seq, last_head = previous.get(seal["stream"], (0, 0, GENESIS))
        if (
            seal["seal"] != number + 1 or seal["firstSeq"] != last_seq + 1
            or seal["prevHash"] != last_head or stored.get(seal["lastSeq"]) != seal["headHash"]
        ):
            report.fail(f"{seal['stream']} seal {seal['seal']} does not continue its chain")
            bad += 1
        previous[seal["stream"]] = (seal["seal"], seal["lastSeq"], seal["headHash"])
    if not bad:
        report.ok(f"{len(seals)} local seals continue each other and match their events")


def check_lineages(anchors, seals, report):
    for stream in STREAMS:
        lines = {}
        for anchor in (a for a in anchors if a["stream"] == stream):
            lines.setdefault(anchor["seal"], set()).add(json.dumps(anchor, sort_keys=True))
        conflicts = sorted(n for n, variants in lines.items() if len(variants) > 1)
        newest = max(lines, default=0)
        gaps = [n for n in range(1, newest + 1) if n not in lines]
        unanchored = [s for s in seals if s["stream"] == stream and s["seal"] > newest]
        report.check(not conflicts, f"{stream}: no anchor_conflict ({len(conflicts)} found)")
        report.check(not gaps, f"{stream}: no anchor_gap ({len(gaps)} found)")
        report.check(not unanchored, f"{stream}: no unanchored seal ({len(unanchored)} found)")


def check_anchors(root, conn, manifest, chains, report):
    anchors = read_anchor_lines(root, report)
    if anchors is None:
        return
    report.check(len(anchors) == manifest["sealCount"], f"anchors.jsonl has sealCount lines ({len(anchors)})")
    foreign = [a for a in anchors if a["installation"] != manifest["installationId"]]
    report.check(not foreign, f"every anchor names this installation ({len(foreign)} others)")
    statuses = {}
    for anchor in anchors:
        status = anchor_status(anchor, chains)
        statuses[status] = statuses.get(status, 0) + 1
        if status != "matched":
            report.fail(f"{anchor['stream']} seal {anchor['seal']}: {status}")
    report.check(set(statuses) <= {"matched"}, f"anchor statuses {dict(sorted(statuses.items()))}")
    seals = local_seals(conn, report)
    if seals is None:
        return
    check_seal_chain(seals, chains, report)
    check_lineages(anchors, seals, report)
    expected = {json.dumps({**s, "format": "vibe-tox-anchor-1", "installation": manifest["installationId"]}, sort_keys=True) for s in seals}
    found = {json.dumps(a, sort_keys=True) for a in anchors}
    report.check(found == expected, f"anchor lines are exactly the {len(seals)} rows of audit_seals")


# 7. Artifact references -----------------------------------------------------------------


def quoted(name):
    return '"' + name.replace('"', '""') + '"'


def check_references(conn, manifest, listed, report):
    references = manifest["artifactReferences"]
    problems = manifest["artifactProblems"]
    if not isinstance(references, list) or not isinstance(problems, list):
        report.fail("artifactReferences and artifactProblems are not lists")
        return
    report.check(not problems, f"artifactProblems is empty ({len(problems)} found)")
    available = {path[len("artifacts/"):] for path in listed if path.startswith("artifacts/")}
    tables = {row[0] for row in conn.execute("SELECT name FROM sqlite_schema WHERE type IN ('table', 'view')")}
    for reference in references:
        table, column = reference.get("table"), reference.get("column")
        columns = {row[1] for row in conn.execute(f"PRAGMA table_info({quoted(table)})")} if table in tables else set()
        if column not in columns:
            report.fail(f"artifact reference {table}.{column} is not a column of the database")
            continue
        values = conn.execute(
            f"SELECT DISTINCT {quoted(column)} FROM {quoted(table)} WHERE {quoted(column)} IS NOT NULL"
        ).fetchall()
        missing = [value for (value,) in values if value not in available]
        report.check(not missing, f"{table}.{column}: {len(values)} referenced artifacts resolve ({len(missing)} missing)")


def verify(root):
    report = Report()
    report.check(hash_matches_test_vector(), "sha256-v1 implementation reproduces the documented test vector")
    manifest = check_manifest(root, report)
    if manifest is None:
        return report
    listed = check_files(root, manifest, report)
    conn = open_database(root, report)
    if conn is None:
        return report
    with conn:
        check_installation(conn, manifest, report)
        check_schema_files(root, conn, manifest, report)
        check_migrations(root, conn, manifest, report)
        chains = check_chains(conn, manifest, report)
        check_anchors(root, conn, manifest, chains, report)
        check_references(conn, manifest, listed, report)
    conn.close()
    return report


def main(argv):
    if len(argv) != 2 or not os.path.isdir(argv[1]):
        print(__doc__.strip().splitlines()[0])
        print("Usage: python3 verify_exit_export.py <package folder>")
        return 2
    try:
        report = verify(argv[1])
    except (sqlite3.Error, ValueError, TypeError, KeyError, AttributeError) as error:
        print(f"FAIL  the package could not be checked ({type(error).__name__})")
        return 1
    if report.problems:
        print(f"RESULT: NOT VERIFIED, {report.problems} problem(s)")
        return 1
    print("RESULT: VERIFIED")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
