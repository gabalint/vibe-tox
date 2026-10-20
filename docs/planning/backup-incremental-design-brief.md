# Incremental backups: design brief

Status: **proposed; design only.** Nothing here is implemented, and implementation waits for the project owner's approval. The decision record is [ADR 0014](../architecture/decisions/0014-incremental-backups.md) (proposed). Sources are in the [engineering ledger](../research/sources-engineering.md#incremental-backup-sources-2026-10-13) (ENG-194–213); capacity and recovery targets are in the [nonfunctional requirements](../requirements/nonfunctional-requirements.md).

## Summary

- **Problem.** Every backup copies the full database. The [current default](../operations/backups-and-recovery-kit.md#schedule-and-retention) is hourly; the 15-minute recovery point (NFR-04, NFR-30) remains an unmet target.
- **Design concern.** `VACUUM INTO` rebuilds the database file (ENG-194), so page-level deduplication needs a stable snapshot source. Actual page churn, database growth and scan cost must be measured on the agreed laboratory workload and reference hardware.
- **Recommendation: option (a), a built-in `vtbk2`.**
  - Every set is a full logical copy that shares unchanged chunks with earlier sets, so there is no chain to break.
  - The database is read page by page inside one read transaction (`sqlite_dbpage`). Pages are grouped into fixed 256 KiB chunks.
  - Each chunk is encrypted as a `vtbk1` file under a keyed name, and only chunks the target lacks are written.
  - Lists of chunks and artifacts are stored as content-addressed blocks, and the manifest is written last.
  - Retention deletes sets; a mark-and-sweep pass deletes unreferenced chunks.
- **Cost model.** Target writes depend on changed chunks; every backup still reads and hashes the database. The proposed design has no accepted capacity or recovery measurement. An optional later step removes the full scan (§6.12).
- **Not recommended:** WAL shipping (b), `rustic_core` (c), a Kopia sidecar (d) and SQLite sessions (e); see §4 and the [ADR 0014 alternatives](../architecture/decisions/0014-incremental-backups.md#alternatives).

## 1. Current limitation

A backup is `VACUUM INTO` a staging file, `integrity_check`, then one `vtbk1` encrypted copy of the whole file with authenticated read-back; artifacts are content-addressed and copied once each ([backup contract](../architecture/operations-contract.md#backup-contract), [formats](../architecture/backup-and-export-formats.md)). Full-copy I/O and retained-set size grow with the database, so a shorter interval needs measurements of workload, database size and target speed first. The hourly default and its [retention](../operations/backups-and-recovery-kit.md#schedule-and-retention) do not meet a 15-minute recovery objective; actual loss after a host failure also depends on the newest available off-host copy.

## 2. Goal and constraints

**Goal:** a recovery point of 15 minutes or less while the app is open. Each copy should cost about what changed, at 5–15 GB databases plus a multi-GB artifact store.

| Constraint | Source |
| --- | --- |
| Restore onto a replacement computer within 4 hours of starting the restore | NFR-30, NFR-04 |
| Encryption on the target, with keys derived from the installation key as in `vtbk1` | [Key custody](../architecture/operations-contract.md#keys-and-recovery-kit), [formats](../architecture/backup-and-export-formats.md) |
| Authenticated read-back before a set counts as verified | [Backup and export formats](../architecture/backup-and-export-formats.md) |
| Manifests bound to their set IDs | [Backup and export formats](../architecture/backup-and-export-formats.md) |
| Sets published but not recorded before a power loss are quarantined at start | [Backup contract](../architecture/operations-contract.md#backup-contract) |
| A restore is applied before the database opens | [Restore contract](../architecture/operations-contract.md#restore-and-reconciliation) |
| Restore tests, the recovery kit, and audit seals and anchors keep working | [Recovery and anchoring](../architecture/operations-contract.md); NFR-18 |
| Retention and garbage collection never remove manual or pre-update sets | [Backup retention](../architecture/operations-contract.md#backup-contract); NFR-11 |
| Folder targets now; write-once buckets later, for optional external backup targets | [Folder targets](../architecture/operations-contract.md#backup-contract); NFR-20; ADR 0010 |
| Windows, macOS and Linux | ADR 0012 |
| No sidecar process without reviewing per-OS packaging/signing (Q-19), process supervision and secret custody | [Operations contract](../architecture/operations-contract.md#backup-contract) |
| The [library policy](../governance/library-policy.md) | Library policy |

## 3. What the evidence says

**SQLite.**
- **`VACUUM INTO` rebuilds the database.** Its copy is "a consistent snapshot", "fully vacuumed", and built by logical copying, so pages move between copies (ENG-194). The amount of churn needs workload-specific measurement.
- **The online backup API keeps the page layout.** It makes "a bit-wise identical copy". Stepping in parts restarts whenever another connection writes, and `step(-1)` holds one read for the whole copy (ENG-195). It always writes every page to its destination. That is 5–15 GB of local writes per copy, which a 15-minute cadence makes a disk-wear problem.
- **`sqlite_dbpage` reads raw pages through the pager.** Inside a read transaction it therefore sees one consistent snapshot. It needs `SQLITE_ENABLE_DBPAGE_VTAB`, which the bundled build does not set, but which `LIBSQLITE3_FLAGS` can add (ENG-199, ENG-200). SQLite's own `sqlite3_rsync` finds changes by hashing pages read this way (ENG-196).
- **No changed-page bitmap was found** (ENG-199). The WAL frame headers list the changed pages, but only until a checkpoint resets the log. Tailing the WAL means controlling every checkpoint, as Litestream does (ENG-197, ENG-201). Reading the database file directly with ordinary file I/O risks dropping SQLite's POSIX locks (ENG-197).
- **Sessions are a poor fit.** They capture row changes of one connection, only for tables with an explicit primary key (all 155 tables here have one), and no DDL (ENG-198). Applying a changeset is ordinary SQL on the target, so the schema's triggers (351 trigger lines in the migrations) fire. Whether apply can suppress them was not established. The `session` feature needs libclang at build time (ENG-200).

**Tools.**
- **Litestream 0.5.17** still has no client-side encryption and no official Windows support, and it needs delete rights (ENG-201).
- **Kopia 0.23.1** is mature. It fixed a data-loss race in that release, its macOS CLI is signed but not notarized, and the password goes in an environment variable or a file. It has open Object Lock bugs (ENG-202, ENG-203).
- **restic 0.19.1** does not support Object Lock, and prune needs delete rights (ENG-204).
- **`rustic_core` 0.13.0** is "in an early development stage", brings about 169 crates (376–397 with backends), and has no Object Lock support (ENG-205).
- **FastCDC** is fast, but no source shows content-defined chunking helping page-structured databases (ENG-206).

**Write-once storage.**
- **Retention.** Compliance-mode locks can be extended but never shortened, and delete markers are not protected (ENG-208, ENG-210).
- **Create-only writes.** Conditional writes give create-only names on S3, GCS and Azure (ENG-209).
- **No multi-object atomicity.** A set is published by writing its objects first and its manifest last (ENG-212).
- **Shared chunks.** A chunk shared by many sets must stay locked as long as the longest-lived set that uses it. Veeam pads locks by a "block generation" period (ENG-211). Kopia renews every blob (ENG-203).

## 4. Options

| | Option | In one line |
| --- | --- | --- |
| (a) | Built-in `vtbk2` with chunk deduplication | Each set is a full logical copy made of content-addressed encrypted chunks; only chunks the target lacks are written |
| (b) | Built-in WAL-frame shipping | Copy the WAL frames committed since the last copy as an encrypted increment; restore is a base plus increments |
| (c) | `rustic_core` | Feed the database and artifacts to a restic-format repository through the library |
| (d) | Kopia sidecar | Bundle the Kopia CLI, stream the database to it and let it deduplicate and encrypt |
| (e) | SQLite session changesets | Record row changes on the writer connection; ship changesets as increments over a base |

**The snapshot source matters more than the tool.** Options (a), (c) and (d) deduplicate chunks, and that works only if unchanged data sits at the same offsets in successive copies.
- `VACUUM INTO` breaks this (§3), so all three need a page-stable source.
- Through the backup API, a page-stable source means a full local write every copy.
- Through `sqlite_dbpage`, it means a full local read and no write. The recommended design uses this.
- Fixed page-aligned chunks fit a page-stable source because SQLite rewrites pages in place. Content-defined chunking is for content that shifts, which a page-stable source does not (an inference; ENG-206 found no source either way).

### 4.1 Comparison

| | Size and dependencies | Failure and restore | Write-once buckets | Constraint conflicts (§2) |
| --- | --- | --- | --- | --- |
| (a) | About 700–1,000 lines of non-test Rust in `lis-ops`, reusing `format.rs`, targets, retention, quarantine and the restore steps. No new crate; `LIBSQLITE3_FLAGS=-DSQLITE_ENABLE_DBPAGE_VTAB` in the workspace build | No chains. An offline target catches up by writing the chunks it lacks. Power loss leaves harmless orphans; the manifest is last and unrecorded sets are quarantined. A damaged chunk damages every set using it until a scrub or restore test finds it and it is repaired from another target. Restore reads one set's chunks, whatever the history | Good: create-only content-addressed objects, manifest last; shared chunks need locks per generation (§6.11); garbage collection by lifecycle expiry, never by the lab computer | None known; restore time unqualified (§6.13) |
| (b) | About 1,000–1,500 lines: WAL parser (salts, checksums, commit frames), checkpoint control in `lis-store` for every connection, chains, compaction, base scheduling | Chains break on a lost increment, an uncontrolled checkpoint or a salt reset. An offline target holds back checkpoints and grows the WAL. Restore replays a base plus increments, so bases and compaction are required | Good for increments; compaction needs deletes or lifecycle; one expired increment breaks the chain | Windows file sharing on `-wal` unverified |
| (c) | About 400–600 lines of adapter; `rustic_core` (and `rustic_backend` for buckets): about 169–397 crates, API "subject to change"; `vtbk1`, the reference decryptor and our manifest semantics give way to the restic format | No chains; packs, then index, then snapshot (ENG-204). Repair is manual; `check --read-data` instead of per-copy read-back. The restic CLI can read the repository (an exit benefit) | Poor today: no Object Lock support; locks and prune need deletes (ENG-204, ENG-205) | Fails the library policy's "modest transitive tree"; quarantine needs rework; Windows issues open (ENG-205) |
| (d) | About 400–600 lines of adapter and supervision; a Go binary per OS to sign and notarize (Q-19); the password passes through its environment or a file | No chains; Kopia keeps the repository consistent and collects incomplete snapshots (ENG-203); `snapshot verify` instead of per-copy read-back | Works with S3 compliance mode and lock extension; open bugs (ENG-203) | A sidecar; quarantine needs rework |
| (e) | About 600–900 lines plus rebase logic; `rusqlite` `session` feature, so libclang (`bindgen`) on every build machine and CI runner | A new base after every migration and unclean stop (no DDL capture, in-memory session state). Restore replays SQL through indexes and triggers: the slowest | As (b) | Build-time libclang |

Key loss is the same for every option: sets are unreadable without the recovery kit. Options (c) and (d) also hold the derived key as a password. No option has a qualified restore time against NFR-30.

## 5. Recommendation

**Option (a): a built-in `vtbk2`.** It is the proposed option for avoiding a chain, sidecar or large additional dependency tree. These design properties do not establish capacity, correctness or restore-time qualification. It reuses the `vtbk1` encryption, the target layout, retention, quarantine and the restore steps.

It costs a full read and hash of the database on every copy. Measure that cost, including the read transaction and checkpoint impact, on reference hardware. §6.12 proposes removing the full scan if it is too slow.

Option (b) becomes interesting only if the laboratory needs a recovery point of seconds, which nobody has asked for.

## 6. The `vtbk2` design

### 6.1 Source

- **One read transaction.** The job opens its own read-only connection, as `snapshot.rs` does today, starts a read transaction, and reads `SELECT pgno, data FROM sqlite_dbpage('main')` in page order. The same transaction reads the heads, the newest seals and the migration ledger, as the current full-copy backup does from its snapshot.
- **No staging file.** Nothing is written to the data folder. The writer continues, and checkpoints wait only for this reader (ENG-197).
- **Build flag.** `SQLITE_ENABLE_DBPAGE_VTAB` is set through `LIBSQLITE3_FLAGS` in the workspace's `.cargo/config.toml`. The virtual table can also write pages, so the job's connection is opened read-only and `query_only`. `SQLITE_DBCONFIG_DEFENSIVE` would refuse writes as well (ENG-199).

### 6.2 Chunks

- **Fixed and page-aligned.** Chunk *i* holds pages 64·*i* + 1 to 64·*i* + 64: **256 KiB** at the 4 KiB page size. The last chunk may be shorter.
- **Proposed chunk size.** 256 KiB is a candidate tradeoff between object count and write amplification. Compare 64 KiB, 256 KiB and 1 MiB against the laboratory workload before acceptance; no size is qualified by this design document.
- **Change detection.** The job hashes every chunk with SHA-256, the approved hash, derives its name (§6.3) and writes the chunks the target lacks. No previous list is needed.

### 6.3 Names and encryption

- **Name.** `chunkName = hex(SHA-256(chunkKey ‖ SHA-256(plaintext)))`, with `chunkKey = SHA-256("vibe-tox/chunk-key/v1" ‖ installation key)`.
  - The key stops anyone with target access from confirming a guessed page by its name.
  - The inner hash avoids length-extension questions.
  - Every target uses the same names, so an object can be copied between targets to repair one.
- **Container.** Each object is a `vtbk1` file ([format](../architecture/backup-and-export-formats.md)) with a fresh file key wrapped by the KEK. Two content codes are added: 4 for a database chunk and 5 for a list block.
  - The container, the STREAM layout and the reference decryptor do not change.
  - Overhead is the 88-byte header plus a 16-byte tag per 1 MiB, under 0.05% at 256 KiB.
- **Write-once.** An object is never rewritten. It is written to a temporary name, synced, renamed, and the folder synced, as artifacts are today.
- **Sharding.** Objects live in `chunks/<first two hex digits>/<name>.vtbk`. Each folder then holds about 240 files at 15 GB, which keeps folder listings short on every file system, FAT-family drives included.

### 6.4 Lists as blocks

A set's manifest must not grow with the database or the artifact count.
- **The scaling problem.** A plain list of 61,440 chunk names would add about 4 MB to every manifest, and 40 manifests a day (15-minute copies over a 10-hour day) would make that a cost proportional to size.
- **Fix: lists as content-addressed blocks.** The chunk list (in page order) and the artifact list are each cut into blocks of 2,048 entries.
  - Each block is stored like a chunk (content code 5).
  - A new set writes only the blocks whose entries changed: at 15 GB at most 30 blocks of 64 KiB, usually a few.
  - The artifact list keeps the order in which artifacts were first copied to the target, so new artifacts land in the last block.
  - The manifest holds only the block names.
- **The `vtbk1` manifest has the same problem.** It lists every artifact in every set. With 25,000 reports a year at about 90 bytes per entry, that is about 2 MB per manifest, or about 90 MB a day at 40 sets a day (arithmetic). This design fixes it too.

### 6.5 Manifest `vibe-tox-set-2`

```
{"format":"vibe-tox-set-2", installationId, setId, runId, kind, createdAt, appVersion, keyId,
 schema:[{module,version,name,checksum}],
 database:{pageSize, pageCount, chunkPages:64, root, integrity:"ok"|"not_checked", checkedAt},
 chunkLists:[name…], artifactLists:[name…], artifactCount,
 heads:[{stream,seq,hash}], seals:[{stream,sealNo,lastSeq,headHash}],
 artifactProblems:[{sha256,problem}]}
```

- `root` is SHA-256 over the ordered chunk names. It identifies the database copy the way the snapshot digest does in `vtbk1`.
- The manifest is encrypted (content 3) and written last, under `sets/<setId>/manifest.vtbk`. The set ID stays in the folder name and in the manifest, so manifest/set-ID binding is retained.
- No `database.vtbk` exists in a `vtbk2` set. The restore code accepts both versions, so existing `vtbk1` sets stay restorable and nothing is converted.

### 6.6 One run

The proposed replacement for the [full-copy run](../architecture/operations-contract.md#backup-contract) is:
1. **Due targets:** unchanged.
2. **Insert the run, seal:** unchanged.
3. **Space check:** the data folder no longer needs room for a snapshot. The target needs room for the changed chunks, at most the database size for a new target.
4. **Snapshot** is replaced by the read transaction of §6.1. `integrity_check` runs on the schedule in §6.10, not on every copy.
5. **Chunk.** For each due target, list its shard folders once. Write every chunk whose name is missing or whose size is wrong, then the new list blocks, then the artifacts as today.
6. **Verify.** Read back and authenticate every object written in this run and compare plaintext digests. Then confirm that every referenced older object exists with its expected size, using one listing per shard folder. Then write the manifest, read it back, decrypt it and compare it with the copy in memory.
7. **Publish.** Rename `sets/<setId>.tmp` into place and record the copy. Only then does the set count. Anchors, retention (§6.9) and the `ops.backup` event follow, unchanged.

Listing 256 shard folders per target per copy is the only cost that grows with the object count. A per-target cache of known names in an `ops_` table is an optimization to measure, not a requirement.

### 6.7 Verification levels

| Level | `vtbk1` | `vtbk2` |
| --- | --- | --- |
| Verified copy | Snapshot intact; every file read back | Every new object read back and authenticated; every referenced object present at its size; manifest read back |
| Scrub (new) | Not applicable | Every object on one target read and authenticated, on a schedule (proposal: weekly, one target per run). A damaged object is moved to `quarantine/`, so the next copy writes it again; the health check `artifact_store` gains a `chunk_store` sibling |
| Restore test | Decrypt, open, chains, anchors, artifacts | The same, assembling the database from chunks; the finding codes `chunk_missing` and `chunk_corrupt` are added |

### 6.8 Power loss and quarantine

- **Orphan objects.** Objects without a manifest are orphans, which garbage collection removes after its safety margin.
- **Unrecorded sets.** A published set without a copy row is moved to quarantine at start, as the current backup reconciliation does.
- **Quarantined manifests.** They stay roots for garbage collection until someone deletes them, so their chunks remain.
- **Temporary files.** `.tmp` objects are cleaned at the next run, as staging is now.

### 6.9 Retention and garbage collection

- **Retention** chooses the sets to keep exactly as today (`retention.rs`). Manual and pre-update sets are always kept.
- **Garbage collection is mark and sweep per target**, after pruning:
  - **Roots.** Every kept set, every manual and pre-update set, and every quarantined manifest.
  - **Mark.** The union of the roots' list blocks and their chunks, read from the list blocks; shared blocks are read once.
  - **Sweep.** Delete unmarked objects older than a safety margin (proposal: 24 hours, like Kopia's, ENG-202).
  - **Never in parallel.** It runs inside the job runner, which runs one job at a time, so no copy can be in flight.
  - **Fail safe.** If any root's manifest or list cannot be read, the pass deletes nothing and raises a warning.
- **Space.** Target space becomes about the database size plus the chunks changed during the retention window. Measure the retention-window working set on the actual workload.
- **Proposed defaults with `vtbk2`** (laboratory question LB-2):
  - interval of 15 minutes;
  - keep 40 recent sets, about one 10-hour working day;
  - 14 daily and 8 weekly sets, a proposed increase from the current 7 daily and 4 weekly.

### 6.10 Restore, restore test, kit, anchors and integrity checks

- **Restore and restore test.** Staging assembles `lis.sqlite3` from the chunks in page order into `ops/restore-staging/`, checking each chunk's name and the `root`. It then continues with the existing [restore verification and apply contract](../architecture/operations-contract.md#restore-and-reconciliation). Restore stays before `Database::open`.
- **Recovery kit.** Unchanged: the installation key derives the KEK, the key ID and now `chunkKey`. The kit's restore steps do not change.
- **Seals and anchors.** Seals run before every set, and anchors at least every 15 minutes. The manifest carries the heads and seals as today, so anchor coverage and the lost-interval rules are unchanged.
- **Integrity check of the live database.** It runs on the reader's snapshot. Its duration must be measured before choosing the verification interval.
  - Proposal: the first set of each hour runs it, and the set records `integrity:"ok"` with `checkedAt`. The other sets say `not_checked` and point to the last check. A failure raises the database-damage alert as today.
  - The laboratory may prefer a different cadence (LB-3).

### 6.11 Write-once buckets later

The bucket adapter writes the same objects with create-only writes (ENG-209) and a compliance-mode lock set at upload (ENG-208).
- **Chunk generations.** A chunk referenced by a kept set must stay locked as long as that set, so chunk names carry a generation: `g = floor(days since installation / G)`.
  - A new generation re-uploads every chunk once (a full copy every G days; proposal: G = 30).
  - Objects of generation `g` get a lock of retention + G, which follows Veeam's published scheme (ENG-211).
  - Scheduled sets therefore need no lock extension.
- **Manual and pre-update sets.** These are the one exception: when such a set is made, the locks of the objects it references are extended once, to the laboratory's retention for them (Q-14).
- **The lab computer's credentials** can put, get, list and extend retention. They cannot delete, bypass governance or change lifecycle rules. A lifecycle rule removes old generations once their locks expire. A hosted-backup provider's or the lab's separate principal handles anything else.
- **The 15-minute copies stay small.** Only the generation change costs a full upload.

### 6.12 Optional later step: changed pages from the WAL

If reference hardware shows the full read too slow at 15 GB:
- The job takes over checkpointing (`wal_autocheckpoint` off; the job checkpoints after each copy).
- It reads the changed page numbers from the WAL frame headers since the last copy, and hashes only those chunks.
- Any checkpoint the job did not run, or a WAL reset it cannot account for, falls back to the full read.
- The target format does not change.

This is the useful part of option (b), as a detector rather than a shipping format. It needs the `-wal` read hazard settled first (ENG-197).

### 6.13 Cost model and qualification

| Per copy | Full copy (`vtbk1`) | Proposed `vtbk2` |
| --- | --- | --- |
| Local writes | Full database snapshot | No staging file |
| Local reads | Snapshot, integrity check and encryption input | One pass over the database for hashing |
| Written to each target | Full database copy | Missing chunks and manifest/list blocks |
| Restore reads | One database file | Every chunk referenced by the selected set |

Measure database growth, page churn, backup/restore duration and writer/checkpoint impact on the agreed workload and each reference host. Restore must include decryption, integrity checks, opening, and audit-chain/anchor verification within NFR-30; a theoretical transfer-rate calculation is not acceptance evidence.

## 7. Milestones and tests

| Milestone | Content | Exit tests |
| --- | --- | --- |
| **B1. Format** | Format document section and reference decryptor update (content codes 4 and 5, list blocks, `vibe-tox-set-2`, chunk names); `vtbk2` writer and reader in `lis-ops`; the `DBPAGE_VTAB` build flag | Property tests: chunk and list round trip, a page-for-page match of an assembled database against a backup-API copy taken with the writer paused, wrong key, flipped bit, truncated object, swapped list block, manifest moved to another set ID |
| **B2. Runs** | Backup run on `vtbk2` (§6.6), verification (§6.7), quarantine, garbage collection, the integrity-check cadence; `vtbk1` stays readable | A power cut at every step: object, list, manifest, rename, record. Unrecorded sets are quarantined and orphans collected only after the margin. Garbage collection never removes an object reachable from a manual, pre-update or quarantined set (property test over random histories). An unreadable root makes the pass delete nothing. A target offline for a week catches up |
| **B3. Restore** | Restore and restore test from `vtbk2`; scrub; health signals; 15-minute defaults only after measurement | The [restore, offline-day and interrupted-copy scenarios](../architecture/operations-contract.md#verification-expectations) on `vtbk2`. A `chunk_corrupt` and a `chunk_missing` finding each for its injected fault. Repair by copying an object from another target. A mixed `vtbk1` and `vtbk2` history restores either set. The perf harness's volume scenario reports per-copy bytes at a year |
| **B4. Measure** | Reference hardware per OS (G2-27) | 15-minute copies at a year; scan time; restore time against NFR-30 |
| **B5. Buckets** (later) | Create-only writes, generations, locks, no delete credential | Deleting or shortening retention with the computer's credentials fails (NFR-20); a restore from the bucket with the computer assumed compromised |
| **B6. Optional** | WAL-assisted change detection (§6.12) | A checkpoint run behind the job's back falls back to a full read |

## 8. Laboratory questions

| # | Question | Proposed default |
| --- | --- | --- |
| LB-1 | Is a 15-minute recovery point while the app is open enough for each data class? Analyzers can resend; manual entries cannot | 15 minutes (NFR-04 as proposed) |
| LB-2 | Retention with cheap copies: how many 15-minute, daily and weekly sets per target? | 40 recent, 14 daily, 8 weekly |
| LB-3 | How often to run a full integrity check of the live database: hourly, daily, or at every copy despite the cost? | Hourly |
| LB-4 | Scrub cadence: how often to read every object on a target? | Weekly, one target per run |
| LB-5 | Is a full read of the database every 15 minutes acceptable on the lab's computer during working hours? | Yes, if reference hardware keeps it under about 30 s |
| LB-6 | For write-once buckets: lock period, generation length, and who holds the delete and lifecycle rights (never the lab computer) | Retention + 30 days; 30-day generations; a hosted-backup provider or a separate lab principal |
| LB-7 | Manual and pre-update sets on a bucket: how long are they locked? | Per the lab's retention schedule (Q-14) |

## 9. Risks and open engineering questions

- **R1. Shared chunks multiply damage.** One damaged object affects every set that uses it. Mitigations: per-copy read-back of new objects, scrub, restore tests, and repair from another target. More than one target stays the recommendation.
- **R2. The scan reads the whole database every copy.** Duration grows with database size and host/storage performance. Measure on reference hardware (B4); §6.12 is the fallback.
- **R3. The build flag** must reach every build: CI, packaging and developer machines. A missing flag must fail loudly at start (the virtual table does not exist), never silently fall back to full copies.
- **R4. `sqlite_dbpage` can write.** Only the backup job's own read-only, `query_only` connection uses it. A test asserts that a write through it fails.
- **R5. Reading through the pager for 15 GB inside one read transaction** holds back checkpoints for the scan's duration, so the WAL grows meanwhile. Measure.
- **R6. Garbage collection is new deletion code.** It is the main data-loss risk in deduplicating tools (ENG-202, ENG-203). It is fail-safe by design (§6.9) and gets property tests.
- **R7. Integrity checks no longer run on every copy.** Damage is found within the check interval instead (LB-3).
- **R8. Compression is not proposed.** SQLite pages of text often compress, but no approved crate exists, and the `zstd` crate's licence changed in 0.14 (ENG-207). Measure the gain before asking.

## 10. Acceptance and integration prerequisites

Before implementation, accept ADR 0014 and resolve the applicable laboratory questions above. The change then needs an approved library/build-flag decision, a versioned format and independent reader, compatible restore of existing `vtbk1` sets, adversarial garbage-collection/crash tests, measured capacity and replacement-host recovery, and current operator/help guidance. The implementation plan and release gates must record the resulting scope and evidence. Object-storage targets still require a separate service/credential-custody decision.
