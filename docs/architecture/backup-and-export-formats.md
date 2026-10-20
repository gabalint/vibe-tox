# Backup and export formats

Status: implemented. This document specifies the files the application writes for backup, exit and tamper evidence, so that they can be read and checked without the application. Design context: [M4 design brief](../planning/m4-design-brief.md) §3 to §5, [ADR 0008](decisions/0008-tamper-evident-audit-and-immutable-recovery.md), NFR-12 and NFR-18. Each section names the code that is the one home of its format; if the two ever differ, the code is wrong or this document is, and both are corrected together.

Sections: [audit anchors](#audit-anchors-vibe-tox-anchor-1), [encrypted backup files](#encrypted-backup-files-vtbk1), [backup target layout](#backup-target-layout-vibe-tox-target-1-and-vibe-tox-set-1), [recovery kit](#recovery-kit-vtkit-1), [restore](#restore-vibe-tox-restore-1) and [exit export](#exit-export-vibe-tox-exit-1).

## Audit anchors (`vibe-tox-anchor-1`)

Code home: `crates/lis-audit/src/anchor.rs` (line format and comparison), `crates/lis-audit/src/seal.rs` (seals), `crates/lis-audit/src/chain.rs` (the `sha256-v1` hash chain, normative) and `crates/lis-ops/src/anchors/` (anchor files on targets, lag, and the explanation of restores).

### What an anchor is

The application keeps three audit streams, `clinical`, `security` and `access`. Each is a hash chain: every event has a sequence number from 1 without gaps and stores the hash of its predecessor. At intervals the application **seals** each stream that has new events: it records the head of the stream (the newest sequence number and that event's stored hash) in the table `audit_seals`. Seals of a stream form their own chain. An **anchor** is one seal written as a line of text to a place outside the host computer, such as a removable drive or a network share. Because each event hash commits to every event before it, an anchor that matches the local database proves that nothing up to its last sequence number has changed since the anchor left the host.

Anchors carry digests, sequence numbers and times only: no event content, actions, principals, record identifiers or patient data. The installation identifier is a random UUID; the privacy officer confirms that it is not identifying (ADR 0008).

### Line format

An anchor file is [JSON Lines](https://jsonlines.org/): UTF-8, one JSON object per line, each line ending in `\n`. A file is written once (temporary name, sync, rename) and never changed. One line:

```json
{"format":"vibe-tox-anchor-1","installation":"01900000-0000-7000-8000-0000000000aa","stream":"clinical","seal":12,"firstSeq":1201,"lastSeq":1350,"prevHash":"abababababababababababababababababababababababababababababababab","headHash":"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd","sealedAt":"2026-10-13T08:00:00.000000000Z","hashAlg":"sha256-v1"}
```

Every key is required, appears once, and no other key is allowed. The application writes the keys in this order; readers must not depend on the order.

| Key | Type | Meaning |
| --- | --- | --- |
| `format` | string | Always `vibe-tox-anchor-1`. Any other value is a different format. |
| `installation` | string | The installation's identifier: a version 7 UUID in lowercase hyphenated form. A lineage of anchors is one installation's stream. |
| `stream` | string | `clinical`, `security` or `access`. |
| `seal` | integer | The seal number within the stream: 1 for the first seal, then each seal is the previous plus 1. |
| `firstSeq` | integer | The first event sequence number this seal adds: the previous seal's `lastSeq` + 1, or 1. |
| `lastSeq` | integer | The newest event sequence number when the seal was taken; at least `firstSeq`. |
| `prevHash` | string | The previous seal's `headHash`, or 64 zeros for seal 1. This equals the stored hash of event `firstSeq − 1`. |
| `headHash` | string | The stored hash of event `lastSeq`: 64 lowercase hexadecimal digits. |
| `sealedAt` | string | When the seal was taken, by the host clock: RFC 3339 in UTC with exactly nine fractional digits and `Z`, for example `2026-10-13T08:00:00.000000000Z`. |
| `hashAlg` | string | The chain's hash algorithm and serialization: `sha256-v1`. |

A reader rejects a line, rather than skipping a key or guessing, when: it is not one JSON object; a key is missing, repeated or unknown; `format` or `hashAlg` has another value; a number is not an integer, or `seal` or `firstSeq` is below 1, or `lastSeq` is below `firstSeq`; a hash is not 64 lowercase hexadecimal digits; the time is not in the form above; or the UUID is not a lowercase version 7 UUID.

### Anchor files on a target

Every backup target receives anchors, whatever its purpose, including targets that receive anchors only. The application writes them inside the target's own folder:

```text
<chosen folder>/VibeToxLIS/anchors/<YYYY-MM-DD>/<YYYYMMDDTHHMMSSZ>-<exportId>.jsonl
```

- **One file per target and pass.** After sealing, each pass writes one new file to each reachable target, holding every seal that target has not received yet: the streams in the order `clinical`, `security`, `access`, and each stream's seals in seal order. The folder is the UTC day of the pass and the name starts with its UTC time; `exportId` is a version 7 UUID, so names sort by time and never repeat. A pass with nothing new for a target writes nothing there.
- **Written once.** A file is written under the same name followed by `.tmp`, synced, renamed into place, and its folder is synced. It is never changed or deleted afterwards. A file ending in `.tmp` is an interrupted write: readers ignore it.
- **Queued while absent.** While a target cannot be reached (for example a removable drive that is unplugged), its seals wait in the database. When it is reachable again, the next file holds all of them, in order. Each target therefore receives each seal once, and its files together hold every seal from 1 without gaps. If the application stops after writing a file but before recording it, the next pass writes the same seals again; identical lines are one anchor.
- **Never skipped.** If a seal cannot be written as a valid line (for example because the database was edited), nothing is written to that target until the cause is resolved; later seals do not skip past it.
- **Off-host.** Only targets of kind *removable drive* or *network share* count as copies off this computer. The application's anchor lag, for each stream, is the time since the oldest event that no seal written to an active off-host target covers.

### The `sha256-v1` event hash

Each event's `hash` column in `audit_events` is the lowercase hexadecimal SHA-256 of these 20 columns of the same row, in this order: `hash_alg`, `stream`, `seq`, `prev_hash`, `occurred_at`, `recorded_at`, `principal_kind`, `principal_id`, `session_id`, `workstation_id`, `action`, `outcome`, `record_type`, `record_id`, `old_revision`, `new_revision`, `reason`, `authority`, `correlation_id`, `details`. Each is encoded as follows, and the encodings are concatenated:

- a present value: the byte `0x01`, then the length of its UTF-8 bytes as a 4-byte big-endian unsigned integer, then those bytes;
- an absent (SQL `NULL`) value: the single byte `0x00`;
- integers (`seq`, `old_revision`, `new_revision`): their base-10 text, with a leading `-` when negative and no leading zeros or `+`;
- text: exactly as stored.

The first event of a stream has `prev_hash` equal to 64 zeros; every later event's `prev_hash` is the stored `hash` of the event before it. Test vector, from `chain.rs`: a `clinical` event with `seq` 1, the genesis `prev_hash`, both times `2026-10-12T10:00:00.000000000Z`, principal `user` `01900000-0000-7000-8000-000000000001`, session `…0002`, workstation `…0003`, action `specimen.receive`, outcome `succeeded`, record `specimen` `…0004`, no old revision, new revision 1, no reason, authority `accession`, correlation `…0005` and details `{}` (each elided ID is `01900000-0000-7000-8000-00000000000N`) hashes to `ac1f0df2609c488cc06475e88feac1c1ab293969768f2206bd669dfc1cf8d55a`.

### Seals in the database

The table `audit_seals` holds one row per seal, with exactly these columns: `stream`, `seal_no`, `first_seq`, `last_seq`, `prev_hash`, `head_hash`, `sealed_at` and `hash_alg`; its primary key is (`stream`, `seal_no`). A seal's anchor line is that row with `format` and `installation` added: `stream` is `stream`, `seal` is `seal_no`, `firstSeq` is `first_seq`, `lastSeq` is `last_seq`, `prevHash` is `prev_hash`, `headHash` is `head_hash`, `sealedAt` is `sealed_at` and `hashAlg` is `hash_alg`. Every value is copied exactly as stored: `sealed_at` is stored in the time form of `sealedAt`, so the two texts are equal.

### Verifying anchors against an exported database

Only the database file (`audit_events` and `audit_seals`), the anchor lines and the hash definition above are needed.

1. **Parse** every line strictly. Keep only lines whose `installation` is the database's installation (`ops_installation.installation_id`); report the others.
2. **Recompute each chain.** For each stream, read the events in `seq` order, recompute every hash and check every `prev_hash` link. Note the lowest sequence number at which a problem appears: a hash that does not match, a broken link, a missing sequence number, or an unknown `hash_alg`.
3. **Check each anchor** (stream `s`, `firstSeq` F, `lastSeq` L, `prevHash` P, `headHash` H):

   | Condition | Status |
   | --- | --- |
   | Event `(s, L)` exists with stored hash H; event `(s, F − 1)` has stored hash P (or F = 1 and P is 64 zeros); and no chain problem appears at or before L | `matched` |
   | Event `(s, L)` does not exist | `local_events_missing`: the stream was truncated, or an older database was swapped in |
   | Otherwise | `anchor_mismatch`: events were edited, and possibly the chain recomputed |

4. **Check each lineage** (one installation's stream):
   - seal numbers between 1 and the newest anchored seal that no anchor covers are `anchor_gap`;
   - two different lines for the same seal number are `anchor_conflict`;
   - local seals (`audit_seals`) numbered above the newest anchor are `unanchored`: anchoring lag, not tampering.

Identical lines found on several targets are one anchor. After a restore from an older backup, anchors newer than the restored head give `local_events_missing` or `anchor_mismatch`, and the re-sealed numbers give `anchor_conflict`. The application explains these with its restore records (`lost_interval`, or `unverified_restore_boundary` while the restore event itself is not yet anchored); without those records, treat them as findings to investigate.

The application also checks that every local seal continues the previous one and that its `head_hash` is the stored hash of the event at its `last_seq`. The local seals are no evidence on their own: whoever can rewrite the chain on the host can rewrite them too. Only anchors kept off the host, out of that person's reach, detect such a rewrite, and only up to the newest anchor.

## Encrypted backup files (`vtbk1`)

Code home: `crates/lis-ops/src/format.rs` (normative), with the property tests in `crates/lis-ops/tests/format.rs`. Design: [M4 design brief](../planning/m4-design-brief.md) §3.2 and D2.

Every file the application writes to a backup target, except `target.json` and anchor files, is a `vtbk1` file: the database snapshot, each artifact and each set's manifest. The construction is ChaCha20-Poly1305 ([RFC 8439](https://www.rfc-editor.org/rfc/rfc8439)) in the STREAM layout of RustCrypto's `StreamBE32`, with a random key per file wrapped under a key-encryption key.

### Keys

The installation key is 32 random bytes, kept in the operating system's key store and printed in the [recovery kit](#recovery-kit-vtkit-1). Two values are derived from it with SHA-256 over an ASCII domain label followed by the 32 key bytes:

| Value | Derivation | Use |
| --- | --- | --- |
| `KEK` | `SHA-256("vibe-tox/backup-kek/v1" ‖ key)` | Wraps each file's key |
| `keyId` | first 16 bytes of `SHA-256("vibe-tox/key-id/v1" ‖ key)` | Names the key in headers, `target.json`, manifests and kit records; shown as 32 lowercase hexadecimal digits |

The installation key itself also seals TOTP secrets in the database; backups never use it directly.

### Layout

A file is an 88-byte header followed by one or more chunks.

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 4 | `VTBK` (ASCII) |
| 4 | 1 | Version: `0x01` |
| 5 | 1 | Content: `0x01` database snapshot, `0x02` artifact, `0x03` manifest |
| 6 | 2 | Reserved: `0x0000` |
| 8 | 16 | `keyId` |
| 24 | 12 | `wrapNonce`: random |
| 36 | 48 | `wrappedFileKey`: the 32-byte ciphertext of `fileKey` followed by its 16-byte tag |
| 84 | 4 | `chunkBytes`: unsigned 32-bit big-endian, always `1048576` (1 MiB) |

- `fileKey` is 32 random bytes, new for every file. `wrappedFileKey` = ChaCha20-Poly1305 encryption of `fileKey` under `KEK` with nonce `wrapNonce` and associated data `header[0..24] ‖ "file-key"` (the first 24 header bytes, then the 8 ASCII bytes `file-key`).
- The plaintext is cut into chunks of exactly `chunkBytes` bytes, except the last, which is **shorter** than `chunkBytes` and may be empty. An empty plaintext, or one whose length is a multiple of `chunkBytes`, therefore ends with an empty final chunk.
- Chunk `i` (from 0) is the ChaCha20-Poly1305 encryption of its plaintext under `fileKey`, with the whole 88-byte header as associated data and the 12-byte nonce `7 zero bytes ‖ i as unsigned 32-bit big-endian ‖ 0x01 if this is the final chunk, else 0x00`. It is stored as the ciphertext followed by its 16-byte tag, so every chunk but the last is `chunkBytes + 16` bytes long.
- The file length is `88 + plaintext length + 16 × (⌊plaintext length / chunkBytes⌋ + 1)`.

### Reading

A reader checks the header first: the magic, version, reserved bytes and chunk size; the content kind it expects; and `keyId` against its own key (another value means the file was written under another installation key). It then unwraps `fileKey` and reads chunks of up to `chunkBytes + 16` bytes: a full-length read is a non-final chunk, a shorter one is the final chunk, and the file must end there. Every authentication failure is an error, never skipped: it means a truncated, damaged or altered file, a chunk from another file or position, a missing final chunk or an edited header. Plaintext is trustworthy only once the whole file has been read without error. Two whole files can still be swapped for each other; the [manifest](#manifest) records the SHA-256 of every plaintext, so a reader compares those too.

### Test vector

With the installation key `00 01 02 … 1f` (bytes 0 to 31), `fileKey` `20 21 … 3f`, `wrapNonce` `40 41 … 4b`, content `0x03` and the 31-byte plaintext `vibe-tox synthetic test vector` followed by a line feed, `keyId` is `548b627d52b0809d7630bae666610381` and the whole file (135 bytes) is:

```text
5654424b01030000548b627d52b0809d7630bae666610381404142434445464748494a4b
372e52e284492139d13a9aabb6e832146fd64210da5976231150b4ac7dfb43c42913c1b8
c8b55132e4ac611a38d12b9700100000253eb8d33b71a8769a738db058cf6aecb94da1be
9dcd12efa9c9c2abc27203be5f21795acff9c35c23595d98f36398
```

The unit test `known_answer_vector_matches_the_format_document` in `format.rs` checks this value. It was also computed independently from this section with Python `cryptography` 48.0 on 2026-10-13, which matched.

### Reference decryptor

Python 3.9 or later with the [`cryptography`](https://cryptography.io/) package. On 2026-10-13 it decrypted files written by the application (empty, an exact multiple of the chunk size, and several chunks) and the test vector above; it is checked by hand, not in CI.

```python
"""Reference decryptor for vtbk1 files. Needs the `cryptography` package.
Usage: python3 vtbk1_decrypt.py "<vtk1-… key line>" <content: 1|2|3> <in.vtbk> <out>"""
import hashlib, struct, sys
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

CHUNK = 1 << 20

def parse_key(text):
    groups = text.strip().lower().removeprefix("vtk1-").split("-")
    key = bytes.fromhex("".join(groups[:16]))
    if len(groups) != 17 or len(key) != 32 or hashlib.sha256(key).hexdigest()[:4] != groups[16]:
        raise ValueError("not a valid vtk1 key line")
    return key

def decrypt(key, content, src, dst):
    kek = hashlib.sha256(b"vibe-tox/backup-kek/v1" + key).digest()
    key_id = hashlib.sha256(b"vibe-tox/key-id/v1" + key).digest()[:16]
    header = src.read(88)
    if len(header) != 88 or header[:5] != b"VTBK\x01" or header[6:8] != b"\0\0" \
            or struct.unpack(">I", header[84:])[0] != CHUNK:
        raise ValueError("not a vtbk1 file")
    if header[5] != content or header[8:24] != key_id:
        raise ValueError("another content kind or another key")
    file_key = ChaCha20Poly1305(kek).decrypt(header[24:36], header[36:84], header[:24] + b"file-key")
    aead, index = ChaCha20Poly1305(file_key), 0
    while True:
        chunk = src.read(CHUNK + 16)
        last = len(chunk) < CHUNK + 16
        nonce = bytes(7) + struct.pack(">I", index) + (b"\x01" if last else b"\x00")
        dst.write(aead.decrypt(nonce, chunk, header))  # raises InvalidTag if altered
        if last:
            return
        index += 1

if __name__ == "__main__":
    with open(sys.argv[3], "rb") as src, open(sys.argv[4], "wb") as dst:
        decrypt(parse_key(sys.argv[1]), int(sys.argv[2]), src, dst)
```

It writes plaintext as it goes; if it raises, delete the output.

## Backup target layout (`vibe-tox-target-1` and `vibe-tox-set-1`)

Code home: `crates/lis-ops/src/targets.rs` (folder and marker), `crates/lis-ops/src/backup/manifest.rs` (sets and manifest), `crates/lis-ops/src/backup/copy.rs` (writing) and `crates/lis-ops/src/backup/retention.rs`. Design: [M4 design brief](../planning/m4-design-brief.md) §3.1, §3.3 to §3.5.

A target is a folder the laboratory chose: a second internal disk, a removable drive or a network share. Only removable drives and network shares count as copies off this computer. The application writes only inside its own subfolder:

```text
<chosen folder>/VibeToxLIS/
  target.json                     the marker; plain text, identifiers only
  sets/<setId>/database.vtbk      the database snapshot (content 0x01)
  sets/<setId>/manifest.vtbk      the set's manifest (content 0x03)
  artifacts/<sha256>.vtbk         one per artifact (content 0x02); written once, shared by sets
  anchors/…                       audit anchors (see above)
```

### Marker

`target.json` is one JSON object with exactly these keys: `format` (`vibe-tox-target-1`), `installationId` (the installation's UUID), `targetId` (the target's UUID, also its database row), `keyId` (32 hexadecimal digits) and `createdAt` (UTC, nine fractional digits). It holds no label, path or patient data. A target is *available* when its folder holds a marker naming it; the application refuses to add a folder inside its data folder, a folder it cannot write, and a folder whose marker names another installation or another key.

### Sets

A set's name `setId` is `<YYYYMMDDTHHMMSSZ>-<8 hex digits>`: the UTC time the run started, then the last 8 hexadecimal digits of the run's UUID. Names sort by time.

A set is written as `sets/<setId>.tmp/`; every newly written encrypted artifact is reopened to compare its ciphertext length and SHA-256 and to authenticate, decrypt and compare its plaintext length and SHA-256. Both set files are synced and read back (their SHA-256 must equal what was written, and the manifest must decrypt to the bytes in memory), and only then is the folder renamed to `sets/<setId>`. A folder ending in `.tmp`, or a file ending in `.tmp` under `artifacts/`, is an interrupted write: readers ignore it and the next run removes it. Artifacts already present are reused only after the same authenticated plaintext check.

Opening a set is one operation: the directory name must have the canonical `setId` syntax above, the manifest must authenticate and parse, and its `setId` must exactly equal that directory name. Listing, retention and restore all use this operation. A copied set under another name, even a newer-looking valid name, is reported as unreadable and is never used. At startup, before listing or retention, the application compares each authenticated final set's `runId` and `setId` with its successful copy record. A final set with no exact record (for example after power failed between publication and recording) moves to the target's `quarantine/` folder and is not used or deleted.

### Manifest

`manifest.vtbk` decrypts to one JSON object (`vibe-tox-set-1`) with exactly these keys:

| Key | Content |
| --- | --- |
| `format` | `vibe-tox-set-1` |
| `installationId` | The installation the set belongs to |
| `setId`, `runId` | The set's folder name and the run that wrote it |
| `kind` | `scheduled`, `manual` or `pre_update` |
| `createdAt` | When the run started (UTC); the snapshot was taken just after |
| `appVersion` | The application version that wrote the set |
| `keyId` | The key the set's files are written under |
| `schema` | The snapshot's migration ledger: `[{module, version, name, checksum}]` |
| `database` | `{sha256, bytes, integrity}`: SHA-256 and length of the decrypted snapshot; `integrity` is always `ok`, because a snapshot that fails SQLite's `PRAGMA integrity_check` is never copied |
| `heads` | The snapshot's audit chain heads: `[{stream, seq, hash}]` for `clinical`, `security` and `access`; an empty stream has `seq` 0 and 64 zeros |
| `seals` | The newest seal of each sealed stream in the snapshot: `[{stream, sealNo, lastSeq, headHash}]` |
| `artifacts` | Every artifact in the set, sorted by digest: `[{sha256, bytes}]`; the file is `artifacts/<sha256>.vtbk` and its plaintext has that SHA-256 and length |
| `artifactProblems` | Artifacts that could not be copied: `[{sha256, problem}]` with `problem` `missing`, `damaged` or `unreadable`; and entries of the artifact folder that are not artifacts, as `{"sha256": null, "problem": "unexpected_entry"}` without their names |

To check a set without the application: decrypt the manifest; decrypt `database.vtbk` and compare its SHA-256 and length with `database`; open it with SQLite and run `PRAGMA integrity_check`; compare its audit chain heads with `heads` (see [the event hash](#the-sha256-v1-event-hash)); and decrypt each listed artifact and compare its SHA-256.

### Retention

After each verified copy, the application deletes older sets of the same target, keeping:
- the newest `keep_recent` sets (default 6);
- the newest set of each of the newest `keep_daily` UTC days that have sets (default 7);
- the newest set of each of the newest `keep_weekly` ISO weeks that have sets (default 4);
- every `manual` and `pre_update` set;
- always the newest set.

The [operations guide](../operations/backups-and-recovery-kit.md#schedule-and-retention) explains the settings. Retention deletes only sets whose manifest decrypts and names this installation. A delete that fails, for example on a write-once share, is logged as a warning and retried at the next run. Retention never deletes artifact files.

## Recovery kit (`vtkit-1`)

Code home: `crates/lis-ops/src/backup/kit_text.rs` (text and key line, normative) and `crates/lis-ops/src/kit.rs` (export records, storage confirmations and the *outdated* status). Design: [M4 design brief](../planning/m4-design-brief.md) §3.3 and §3.9.

The kit is a UTF-8 text file, `vibe-tox-recovery-kit-<YYYY-MM-DD>.txt` by default, that restores the installation key on a replacement computer. That key decrypts the backups and the sealed sign-in secrets. The kit is not itself a sign-in credential and is separate from each person's security-key recovery code ([ADR 0019](decisions/0019-security-key-authentication.md)). **Anyone with this file can read your backups.** Store it like a disk-encryption recovery key, in two places off this computer, and confirm in the application where it is kept at least every 90 days (NFR-30).

It holds, one item per line (`Label: value`): the format `vtkit-1`; the installation ID; the kit number (1, 2, 3 …); when and by whom (sign-in name) it was created; the application version, platform, operating system and data folder; the installation key; the key fingerprint; the laboratory's description of where the disk-encryption recovery key is kept; the backup locations at export time (`- label (kind, purpose): path`); the restore steps; and the two lines `Cloud re-enrollment: not applicable in this version` and `Break-glass envelope: not applicable in this version`.

### Key line

```text
Installation key: vtk1-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-CCCC
```

After `vtk1-` come the 64 hexadecimal digits of the 32 key bytes in 16 groups of 4, then a typing checksum `CCCC`: the first 4 hexadecimal digits of SHA-256 over the 32 key bytes. The application writes lowercase. A reader accepts upper or lower case and surrounding spaces, and refuses anything else: another prefix, another number or length of groups, a non-hexadecimal digit, or a wrong checksum (reported separately, as a typing error). Example: the key `00 01 02 … 1f` is

```text
vtk1-0001-0203-0405-0607-0809-0a0b-0c0d-0e0f-1011-1213-1415-1617-1819-1a1b-1c1d-1e1f-630d
```

The *key fingerprint* line is the `keyId` in 8 groups of 4 hexadecimal digits; it identifies the key without revealing it. The key line is the only line holding key material: the export record and its audit event hold the kit number and `keyId`, never the key.

### Outdated kits

A kit is *outdated* when none was exported, when the installation key's `keyId` differs from the latest export's, or when the active backup locations (their IDs and paths) changed since then. Each export and each storage confirmation is recorded; only the latest kit can be confirmed as stored.

## Restore (`vibe-tox-restore-1`)

Code home: `crates/lis-ops/src/restore/` (`stage.rs` and `checks.rs` for the checks, `findings.rs` for the finding codes, `apply.rs` for the staging folder and markers, `record.rs` for the restore records, `recovery.rs` for the recovery reconciliation). Design: [M4 design brief](../planning/m4-design-brief.md) D4, D5, §3.6 to §3.8.

### Checking a set

A restore test and a real restore check a set in the same way. The set is decrypted into a scratch folder inside the data folder: `ops/restore-tests/<testId>/` for a test, deleted afterwards, or `ops/restore-staging/` for a restore. The checks, in order, and the finding each failure gives:

| Step | Check | Finding |
| --- | --- | --- |
| 1 | The manifest decrypts and parses | `manifest_unreadable` |
| 1 | The manifest and the database file were written under the entered key | `key_mismatch` |
| 1 | Every migration in the manifest's `schema` exists in this build with the same name and checksum | `schema_newer_than_app` |
| 2 | `database.vtbk` decrypts, and its SHA-256 and length equal the manifest's `database` | `database_digest_mismatch` |
| 2 | SQLite's `PRAGMA integrity_check` returns `ok` | `integrity_failed` |
| 3 | The database opens with this build's migrations, exactly as a restore opens it | `open_failed` |
| 4 | Each audit chain verifies (one finding per problem) | `chain_broken` |
| 4 | Each chain head equals the manifest's `heads` | `head_mismatch` |
| 4 | Each seal continues the previous one and names the stored hash of its event, and each stream's newest seal equals the manifest's `seals` | `seal_inconsistent` |
| 5 | The anchors on the target, and on the other folders given, are compared as in [verifying anchors](#verifying-anchors-against-an-exported-database), with the restore records of the restored database | see below |
| 6 | Each artifact in the manifest has its file, which decrypts to the listed SHA-256 and length | `artifact_missing`, `artifact_corrupt` |
| 7 | Every digest the restored database references (in the columns each module declares) is in the manifest's `artifacts` | `referenced_artifact_missing` |

A later step is skipped when an earlier one makes it meaningless: nothing after step 1 runs without a readable manifest, and steps 3 to 5 and the reference check run only on a database that decrypted, matched its digest and passed the integrity check. Artifacts are checked whenever the manifest is readable.

Anchors give these findings:

- an anchor sealed **after** the set was taken whose last sequence number is beyond the set's head of its stream is `lost_interval`: the events the restore will lose. It is expected, and counted;
- an anchor the restore records already explain (see [verifying anchors](#verifying-anchors-against-an-exported-database)) is also `lost_interval`;
- an anchor sealed before the set was taken whose event the set lacks is `local_events_missing`: the set is missing events it should hold;
- any other anchor the chain does not reproduce, two different anchors for one seal that no restore explains, a restore boundary that is not anchored yet, a line that is not a valid anchor line and an unreadable file are `anchor_mismatch`.
- a missing seal number below the newest anchor in a lineage is `anchor_gap` and fails the check; local seals newer than the newest anchor are ordinary unanchored lag and remain informational.

A set **passes** when no finding other than `lost_interval` remains. A restore test records its elapsed time (the measured restore time of NFR-30), its anchor counts and its findings; a test that could not check its set fails with `key_unavailable`, `target_unavailable`, `set_not_found` or `scratch_failed` and no findings.

### Restore onto a new installation

A restore goes only onto an installation without user accounts. After the set passes, confirming writes the marker `ops/restore-staging/READY.json`. The application quits; at its next start, before it opens the database, it applies the restore:

1. It rechecks the staged database's SHA-256, every staged artifact's manifest length and SHA-256, every live artifact it would reuse, and that the current database has no user accounts. If one fails, nothing moves. The expected artifact digests and lengths come from the authenticated manifest and are persisted in the marker. The file checks run again when resuming `APPLYING`.
2. The marker becomes `APPLYING.json`, with the time applying began.
3. The current `lis.sqlite3`, `lis.sqlite3-wal` and `lis.sqlite3-shm` move to `ops/replaced/<YYYYMMDDTHHMMSS.nnnnnnnnnZ>-<restoreId>/`. They are never deleted. A directory collision is refused before anything moves.
4. The staged database moves into place, and each staged artifact moves into `artifacts/`. An artifact already there is kept if its SHA-256 equals its name; otherwise it moves to `ops/replaced/<…>/artifacts/` first.
5. The marker becomes `APPLIED.json`.

Each step can be repeated, so a start after a crash while `APPLYING` resumes; a step never moves the restored database away. A marker moves to its next state by writing the new file and then deleting the old one; a reader that finds two trusts the more advanced one (`APPLIED`, `APPLYING`, `FAILED`, `READY`). After the database opens, one transaction records the restore and deletes nothing; then the staging folder, with its marker, is removed. A start that finds a staging folder without a marker removes it.

The marker is one JSON object with exactly these keys: `format` (`vibe-tox-restore-1`), `restoreId`, `setId`, `setCreatedAt`, `setKind`, `snapshotSha256` (from the manifest), `stagedSha256` (the staged file after its checks), `backupAppVersion`, `keyId`, `stagedAt`, `heads` and `artifacts` (the authenticated manifest's digests and lengths), `anchorCounts` (nine counts: compared, matched, lost interval, unverified restore boundary, mismatched, local events missing, conflicts, gaps, unanchored), `appliedAt`, `replacedDatabaseSha256` and `failure` (the last three `null` until set). It holds no key material and no patient data.

### Restore records

Recording a restore inserts, in the restored database, a row of `ops_restores` (the set, its time and digest, when it was staged, applied and recorded, the digest of the database it replaced, the anchor counts, the number floors and the sequence number and hash of its own `ops.restore_applied` event on the `security` stream) and one row of `ops_restore_heads` per stream with the head it restored. The same transaction raises the accession and container number floors (`spc_number_floors`, by 1,000 and 5,000 above the highest issued numbers) and audits `ops.restore_applied`. These records are what explain a restore's lost interval to anchor verification: an anchor sealed before a restore was applied, beyond that restore's head of its stream, was lost with it.

Every recorded restore opens a recovery reconciliation (`ops_v_recovery_open`): until a quality manager closes it with all eleven checklist items attested (`ops_recovery_attestations`, `ops_recovery_closures`), release, delivery and billing handoff wait and label printing is held.

## Exit export (`vibe-tox-exit-1`)

Code home: `crates/lis-ops/src/exit_export/` (`package.rs` for the layout and manifest, `dictionary.rs` for the schema files). Design: [M4 design brief](../planning/m4-design-brief.md) D16 and §5; REQ-OPS-010 and NFR-12.

This section is also the `README.txt` of every exit package, followed there by the audit anchors section of the same document, so that the package can be read and checked with nothing else.

**This package holds patient data in plain text.** Keep it only on encrypted media, and give it only to people allowed to see the laboratory's records. It is a complete copy of one installation's data at one moment: the database, every stored file (reports, interface messages), the audit trail and its seals, and a description of every table.

### Package layout

```text
vibe-tox-exit-<installationId>-<YYYYMMDDTHHMMSSZ>/
  README.txt                       this text
  manifest.json                    what the package holds, written last
  database/lis.sqlite3             the database (SQLite 3)
  artifacts/<sha256>               every stored file, named by the SHA-256 of its bytes
  audit/anchors.jsonl              every seal of the audit trail, one anchor line each
  tools/verify_export.py           a convenience copy of the verifier (Python standard library)
  schema/schema.sql                every CREATE statement of the database, in creation order
  schema/data-dictionary.json      every table and view, described (format below)
  schema/data-dictionary.md        the same, for reading
  schema/migrations/<module>/<NNNN>_<name>.sql   the source of every migration
```

The folder name holds the installation's ID and the UTC time the export started. A folder of this form **without `manifest.json` is incomplete**: an export was interrupted. Do not use it. The manifest is written last, after every other file is written and synced.

### Manifest

`manifest.json` is one JSON object with exactly these keys:

| Key | Content |
| --- | --- |
| `format` | `vibe-tox-exit-1` |
| `installationId` | The installation's ID (a version 7 UUID); equals `ops_installation.installation_id` in the database |
| `exportId` | This export's ID (a version 7 UUID) |
| `createdAt` | When the export started (UTC); the audit trail was sealed then |
| `appVersion` | The application version that wrote it |
| `schema` | The database's migration ledger, `[{module, version, name, checksum}]`: exactly the rows of the table `store_migrations`, written in the order of `module`, then `version`. Readers compare it with the table as a set of rows and must not depend on the order. |
| `files` | Every file of the package except `manifest.json`: `[{path, sha256, bytes}]`, with `/` as the separator, the lowercase hexadecimal SHA-256 of the file's bytes and its length |
| `heads` | The head of each audit stream, `[{stream, seq, hash}]`: exactly one entry for each of the three streams, written in the order `clinical`, `security`, `access`; the newest sequence number and its stored hash, and for an empty stream `seq` 0 and 64 zeros |
| `sealCount` | The number of lines in `audit/anchors.jsonl`: one per row of `audit_seals`, the streams in the order `clinical`, `security`, `access` and each stream's seals in seal order |
| `artifactReferences` | The columns that hold artifact digests, `[{table, column}]` |
| `artifactProblems` | Reserved problem list. It is empty in a completed package: any missing, damaged, unreadable or unexpected artifact, including a digest named by any declared reference column, fails the export and removes the incomplete package. A verifier rejects a non-empty list. |

### Conventions in the database

- **Tables** carry the prefix of the module that owns them (`spc_` specimens, `res_` results, `ops_` operations, `audit_` audit trail, and so on); `data-dictionary.json` names each table's module.
- **Identifiers** are TEXT: version 7 UUIDs in lowercase hyphenated form, unless a column says otherwise (accession numbers, codes).
- **Times** are TEXT in RFC 3339, UTC, with nine fractional digits and `Z`, for example `2026-10-13T08:00:00.000000000Z`. Where the domain needs a source time zone offset or a precision (a collection known only by date), separate columns hold them.
- **Quantities** are TEXT decimals exactly as reported or entered (for example `0.50`, which is not the same as `0.5`), never floating point. Their unit, comparator (`<`, `>`) and scale are separate columns.
- **Codes** are TEXT from closed lists; each column's `CHECK` constraint in `schema/schema.sql` lists the allowed values.
- **Stored files.** A column listed in `artifactReferences` holds the lowercase hexadecimal SHA-256 of a file; that file is `artifacts/<sha256>`, and the SHA-256 of its bytes equals its name.
- **Evidence is kept, not overwritten.** Evidence tables, such as released report revisions, decisions and audit events, are append-only: their triggers refuse changes and deletions, and a correction is a new row that refers to the earlier one. Mutable records carry a `revision` number.
- **Audit trail.** `audit_events` holds three hash chains (`clinical`, `security`, `access`) in the `sha256-v1` format, and `audit_seals` the seals of each chain; both are specified in the audit anchors section that follows.
- **Migrations.** `store_migrations(module, version, name, checksum, applied_at)` lists every migration applied; `checksum` is the SHA-256 of the file `schema/migrations/<module>/<NNNN>_<name>.sql` (`NNNN` is the version with four digits).

### Data dictionary

`schema/data-dictionary.json` is one JSON object: `{"format": "vibe-tox-dictionary-1", "tables": [...]}`. Each table or view, in name order, has: `name`; `kind` (`table` or `view`); `module` (the owning module, or `null`); `columns`, each with `name`, `type`, `notNull`, `default` (the SQL default expression or `null`), `primaryKey` (its position in the primary key, 0 when not part of it), `generated` and `artifact` (`true` for a column listed in `artifactReferences`). A table column's `type` is its declared type. A view column's `type` is the type the exporting SQLite inferred from the view's query; another SQLite version can infer a different type, so read the view's `ddl` instead; `foreignKeys`, each `{from, table, to}` (`to` is `null` for the referenced table's primary key); `ddl`, the `CREATE` statement, which holds the `CHECK` constraints; `indexes`, their `CREATE` statements; and `triggers`, each `{name, ddl}`. The triggers show which tables refuse changes and deletions.

### Checking a package

Everything below needs only this text, the files of the package and a SQLite library; Python's standard library (`hashlib`, `json`, `sqlite3`) is enough. The embedded `tools/verify_export.py` is only a convenience copy and is data from the package until independently checked. **Do not run it first.** Obtain a trusted `scripts/verify_exit_export.py` from the published Vibe Toxicology repository at the exported `appVersion`, or use the copy installed with the application. The app's export-completion details and the final line of this README print the embedded file's SHA-256. Compute the embedded file's SHA-256 with an operating-system tool (`sha256sum`, `shasum -a 256`, or `Get-FileHash -Algorithm SHA256`) and compare all 64 hexadecimal digits with both the trusted copy and the printed value. Then run the trusted copy from outside the package:

```text
python3 -I /trusted/path/verify_exit_export.py <package folder>
```

Only after its digest matches may the embedded copy be substituted. The verifier prints each check and exits with 0 only when everything verifies. This text, not either tool copy, defines the format.

1. **Manifest.** `manifest.json` parses, `format` is `vibe-tox-exit-1`, every fixed required path is present, migration sources are present, and no other file than those listed and `manifest.json` itself is in the package.
2. **Files.** Each file listed exists, and its SHA-256 and length equal the manifest's.
3. **Database.** `database/lis.sqlite3` opens (read-only is enough) and `PRAGMA integrity_check` returns the single row `ok`. `ops_installation.installation_id` equals `installationId`, and the rows of `store_migrations` equal `schema`.
4. **Schema, dictionaries and migrations.** `schema/schema.sql` exactly describes the database's `sqlite_schema`; both data dictionaries describe the database's tables, views and columns, apart from the version-dependent inferred types of view columns; and for each row of `store_migrations`, the SHA-256 of `schema/migrations/<module>/<NNNN>_<name>.sql` equals `checksum`.
5. **Audit chains.** `audit_events` holds only the three streams; each chain recomputes as the `sha256-v1` definition below states, and its head equals its entry in the manifest's `heads`, which lists exactly the three streams.
6. **Seals and anchors.** `audit/anchors.jsonl` has `sealCount` lines; each is a valid anchor line of this installation and matches the database as the anchor verification below describes, and together they are exactly the rows of `audit_seals`. These lines come from the database itself, so they show only that the copy is consistent. The anchors kept on backup targets and other media, outside the host, are the evidence that the audit trail was not rewritten: check those against the database in the same way. The tables `ops_restores` and `ops_restore_heads` explain anchors lost with a restore (see the formats document, Restore).
7. **References.** `artifactProblems` is empty. For each `{table, column}` of `artifactReferences`, every non-null value is the name of a file under `artifacts/` listed in the manifest.

### How the package is made

The application seals the audit streams; copies the database with SQLite's `VACUUM INTO` and refuses a copy that fails its integrity check; queries every declared artifact-reference column and copies every stored file, checking each file's SHA-256 against its name as it reads it; fails if any stored or referenced artifact is unavailable; writes the other files, each synced; writes `manifest.json` last; and reopens every listed file and the manifest to compare its SHA-256, length and parsed content. If anything fails, it removes the package folder it created. It never writes into a folder it did not create, and it refuses a destination inside its own data folder. The export needs the `export` permission, fresh authentication (password or security key) and a reason, and it is recorded in the audit trail (`ops.request_exit_export`, `ops.exit_export`) without any path or patient data.
