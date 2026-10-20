# ADR 0014: Incremental backups with a built-in chunked format (`vtbk2`)

Status: **proposed**, 2026-10-13. Implementation waits for the project owner's approval.

Design detail: [incremental backup design brief](../../planning/backup-incremental-design-brief.md). Technical sources: the [engineering ledger](../../research/sources-engineering.md#incremental-backup-sources-2026-10-13), ENG-194–213. This ADR refines the backup row of [ADR 0012](0012-rust-tauri-desktop-application.md) and the current [backup contract](../operations-contract.md#backup-contract).

## Context

- **Full copies.** The current application backs up by `VACUUM INTO`, an integrity check and one encrypted full copy per target (`vtbk1`). Artifacts are already content-addressed and copied once each. I/O and target space grow with the database.
- **Recovery objective.** The current full-backup default is hourly; NFR-04 and NFR-30 retain a proposed 15-minute recovery point. Database growth, copy duration and target capacity require measurement on the laboratory workload and reference hardware.
- **Page stability.** `VACUUM INTO` rebuilds the file (ENG-194). A page-deduplicated design needs a stable source; actual churn must be measured before accepting a chunk size or cost claim.
- **Constraints carried over.**
  - Encryption with keys derived from the installation key.
  - Authenticated read-back before a set counts.
  - Manifests bound to set IDs.
  - Quarantine of unrecorded sets.
  - Restore before the database opens.
  - Manual and pre-update sets never pruned.
  - Folder targets now and write-once buckets later.
  - All three desktop OSes.
  - No sidecar without a documented need and dependency/packaging review.
  - The library policy.

## Decision (proposed)

1. **A built-in `vtbk2` replaces whole-file copies of the database.**
   - **Source.** The database is read page by page inside one read transaction through `sqlite_dbpage`, which needs the `SQLITE_ENABLE_DBPAGE_VTAB` build flag. Nothing is staged on the data disk.
   - **Chunks.** Pages are grouped into fixed, page-aligned chunks of 256 KiB.
   - **Names.** Each chunk is named by a keyed hash derived from the installation key.
   - **Encryption.** Each chunk is encrypted in the unchanged `vtbk1` container.
   - **Only missing chunks are written.** The chunks a target lacks are written once and never rewritten.
2. **Every set is a full logical copy.** Its manifest, `vibe-tox-set-2`, names content-addressed list blocks of chunks and artifacts, and is written and read back last. There are no chains, no periodic full copies for correctness and no compaction.
3. **Verification.**
   - A copy counts after its new objects and manifest are read back and authenticated, and every object it references is present.
   - A scheduled scrub reads whole targets.
   - Restore tests assemble and check the full database.
4. **Garbage collection.** Retention keeps choosing sets as today. A fail-safe mark-and-sweep pass then deletes objects that no kept, manual, pre-update or quarantined set references, after a 24-hour margin.
5. **Write-once buckets (later).**
   - Create-only writes with compliance-mode locks.
   - Chunk generations, re-uploaded every N days, are locked for retention plus N days.
   - No delete right on the laboratory computer.
6. **Coexistence.** `vtbk1` sets stay restorable. Exit export and pre-migration snapshots keep using `VACUUM INTO`.

## Alternatives

| Alternative (brief option) | Why not now |
| --- | --- |
| Keep full copies, at 15 minutes | Repeated full database writes; target capacity and duration must be measured |
| Chunk-deduplicate `VACUUM INTO` copies | Rebuilt file layout may reduce deduplication; measure page churn on the chosen workload |
| Page-stable copy through the online backup API | Writes the whole database locally on every copy (ENG-195): disk wear at a 15-minute cadence |
| (b) Built-in WAL-frame shipping, Litestream-style | Takes over checkpoints and needs a WAL parser, chains, periodic bases and compaction (ENG-197, ENG-201). Kept as an optional change *detector* (brief §6.12) |
| (c) `rustic_core` | Early-stage API, a large transitive dependency tree and no Object Lock support (ENG-205) |
| (d) Kopia sidecar | A per-OS binary to sign and notarize, a supervised child process and a stored password; still needs a page-stable source; open Object Lock bugs (ENG-202, ENG-203) |
| (e) SQLite session changesets | No DDL capture and in-memory state force a new base after every migration and unclean stop; replay runs through triggers; libclang at build time (ENG-198, ENG-200) |

## Consequences

- **Cost per copy.** Target writes depend on missing chunks; local work still reads and hashes the whole database. Neither capacity nor timing is qualified by this proposal.
- **Defaults.** The interval default can move to 15 minutes once reference hardware confirms the scan cost (G2-27).
- **Shared-chunk risk.** One damaged object affects every set that shares it. Read-back, scrub, restore tests, repair from another target, and more than one target address it.
- **New deletion code.** Garbage collection is new deletion code and is fail-safe by design. It is the main data-loss risk in comparable tools.
- **Integrity checks.** The live database's `integrity_check` moves from every copy to a schedule (proposed hourly; brief laboratory question LB-3).
- **Format and build changes.** The format document, the reference decryptor and the library policy (one build flag) change.

## Evidence

- Representative workload and reference-host measurements are required before acceptance; current requirements are in [NFR-04/NFR-30](../../requirements/nonfunctional-requirements.md).
- ENG-194–213: SQLite mechanisms (`VACUUM INTO`, the backup API, `sqlite3_rsync`, the WAL, sessions, `sqlite_dbpage`), Litestream, Kopia, restic, `rustic_core`, content-defined chunking, and write-once storage semantics on AWS, Backblaze, Azure, Google Cloud and Wasabi.

## Revisit

- Reference hardware shows the full read too slow at the laboratory's database size: add WAL-assisted change detection (brief §6.12).
- `rustic_core` declares a stable API with Object Lock support, or Litestream ships encryption and Windows support.
- The laboratory needs a recovery point well under 15 minutes.
- Write-once bucket targets are scheduled (brief milestone B5).
