# Local storage, recovery and host evidence

This document connects the local application governed by
[ADR 0009](../architecture/decisions/0009-single-computer-local-lis.md) and
[ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md) to technical
evidence and remaining qualification work. Checked dates and limits are in the
[engineering ledger](sources-engineering.md#single-computer-technology-sources).
Code and operator guides establish implemented behavior; published platform capabilities
and proposed targets do not establish laboratory qualification.

## 1. Runtime topology

The installed application owns the local database and clinical workflow. Its embedded
UI uses in-process Tauri commands. Analyzer listeners run inside the application while
it is open, including at sign-in; there is no background service. The
[desktop platform evidence](desktop-application-stack.md) describes OS-session and
web-engine constraints.

The live SQLite database must stay on a local filesystem: WAL depends on shared memory
between connections on the same host (ENG-97/99). Additional workstations are Q-28;
a network-shared database file is not a substitute for an authorized API. Analyzer
connections and report/label printing require separate qualification of devices, drivers,
formats and recovery behavior ([integration architecture](../architecture/integrations.md)).

## 2. Language and runtime

The [library policy](../governance/library-policy.md) owns dependency choices, and
lockfiles own versions. `rust_decimal` supplies exact arithmetic, while the original lexical
quantity, units, precision and comparator remain distinct record fields. `lis-report`
embeds the pinned Typst renderer; upstream PDF/A and PDF/UA capability (ENG-85/146)
does not prove that a particular report template meets those formats or accessibility needs.

## 3. Database

`lis-store` uses bundled SQLite through `rusqlite`. Its connection and transaction
code implements WAL, `synchronous=FULL`, foreign keys, a busy timeout and a queued
writer using immediate transactions; see `crates/lis-store/src/database.rs`,
`context.rs` and the [transaction contract](../architecture/overview.md#transaction-and-failure-design).

| Constraint | Evidence and implication |
| --- | --- |
| Durability | WAL with `synchronous=FULL` provides stronger power-loss durability than NORMAL, subject to OS and storage behavior (ENG-99). Host qualification must exercise interruption and recovery; a UPS does not replace backup. |
| One writer | SQLite permits one writer at a time. Immediate transactions avoid deferred read-to-write upgrade surprises (ENG-100); parsing and PDF rendering stay outside the short commit transaction. |
| Exact quantities | SQLite has no native decimal storage type. NUMERIC affinity can coerce lexical quantities, so source values use TEXT with separate units, comparator and precision (ENG-98). Lexical ordering is not numeric ordering. |
| Release races | Release rechecks clinical gates, current revisions and authorization in the same transaction as state, audit and outbox writes. Both release/invalidation commit orders need adverse testing (NFR-01). |
| Capacity | The accepted 100–250 patients/day target is not a measured throughput result. Benchmark the agreed specimen, analyte, import and accumulated-history workload against NFR-29 on qualified hardware. |
| Encryption | The shipped bundled SQLite dependency does not enable SQLCipher. Host full-disk encryption and lab-controlled recovery custody remain part of the host baseline; local encrypted backups have a separate format and key contract. |

## 4. Packaging, signing and updates

The [packaging guide](../development/packaging.md) owns the actual NSIS, macOS and
Linux bundle configuration. These are unsigned developer bundles. Publisher identity,
release signing, notarization and an update channel remain Q-19/Q-34. The updater is not
configured.

The retained signing evidence covers certificate validity and hardware-key obligations
(ENG-111), the limits of SmartScreen reputation and SignPath's eligibility/publisher terms
(ENG-112), and conflicting eligibility details in the checked Azure Artifact Signing
pages (ENG-110). None selects or establishes access to a signing service. Recheck the
selected provider's current terms before a release decision.

The required update workflow includes laboratory approval, a verified pre-update backup
and recovery by restoring a matching version/data pair. The current implementation and
remaining gates are in the [update guide](../operations/updates.md) and
[release readiness](../quality/open-items-g1-g2.md). Restoring an older snapshot requires
reconciliation of work committed afterward. Signed-update metadata and OS package
signatures are distinct controls (ENG-113/132).

## 5. Backup and recovery

The implemented `lis-store` snapshot uses `VACUUM INTO`; `lis-ops` writes encrypted full
backup sets with referenced artifacts. The default schedule is hourly. The
[backup guide](../operations/backups-and-recovery-kit.md) owns target setup, retention,
recovery-kit custody and restore tests. The proposed 15-minute snapshot objective
(NFR-04/NFR-30) is unmet; the [incremental backup brief](../planning/backup-incremental-design-brief.md)
owns that open design and its alternatives.

| Property | Evidence and qualification boundary |
| --- | --- |
| Snapshot consistency | SQLite documents `VACUUM INTO` as a consistent snapshot; interrupted output can be incomplete (ENG-101/194). Backups need integrity, artifact-digest and audit-continuity checks. |
| Complete recovery | A recoverable set needs database, referenced artifacts, configuration and usable recovery material. Restore rehearsal on a replacement host establishes more than successful file copying. |
| Off-host custody | A copy kept only on the lab computer does not survive loss of that computer. Windows DPAPI protection can also bind secrets to the original computer/user (ENG-106); keep the independent recovery kit off-host. |
| Ransomware and deletion | Writable local or network copies can be changed by credentials that reach them. Offline media or a separately qualified immutable target protect a different failure boundary. Ordinary writable copies do not establish immutability. |
| Audit evidence | Local hash chains alone do not establish resistance to a privileged administrator. Off-host anchors and their verified lag remain part of ADR 0008 and Q-30. |
| Recovery time | Snapshot interval, time of the latest usable off-host copy, key availability, transfer speed and replacement hardware all matter. Proposed recovery targets need measured exercises, not arithmetic estimates. |

A future managed backup service remains separate scope. Its storage, retention and
restore-verification choices are summarized in [external-service evidence](cloud-services.md#44-managed-backup-for-a-single-pc-lab).
A keyless provider can inspect stored ciphertext and metadata but cannot perform the
same decrypted restore test as the laboratory (Q-31).

## 6. Local-to-cloud connectivity

No cloud connector is implemented or authorized by this study. The proposed boundary
in [ADR 0010](../architecture/decisions/0010-optional-cloud-services.md) keeps the lab
computer authoritative, initiates external connections from the host, separates durable
receipt from application acceptance, and requires retry-safe, ordered reconciliation.
[Integrations](../architecture/integrations.md) owns the protocol proposal; device
identity, enrollment, revocation and restored-host fencing remain qualification work.
The technical sources for device credentials, idempotency and transactional outboxes
are ENG-121–124. A standard or provider example does not qualify an installation.

## 7. Cloud services stack

No hosting runtime, provider or managed product is selected (Q-09). The
[proposed boundary](../architecture/decisions/0010-optional-cloud-services.md) and its
[evidence](cloud-services.md) keep clinical rules and authoritative state local.

## 8. Local authentication on a shared bench PC

Individual LIS attribution is independent of the OS login. A key touch proves presence,
not user verification or two factors. The accepted routes are in
[ADR 0019](../architecture/decisions/0019-security-key-authentication.md) and their
evidence in the [security-key research](security-key-authentication.md); physical-key,
OS, recovery and restored-credential qualification remains Q-42.

## Failure behavior

| Failure | Current design or remaining control to qualify |
| --- | --- |
| Power loss or app exit | Recover database state, reconcile interrupted jobs and receipts, and verify analyzer replay. No background process continues while the app is closed. |
| Disk failure or host loss | Restore from a usable off-host backup and independent recovery kit. The loss window is the age of that copy, not the proposed snapshot target. |
| Ransomware | Rebuild a trustworthy host and restore a separately protected copy. Local writable backups alone do not satisfy this failure case. |
| Privileged tampering | Verify audit chains against independently retained anchors; disclose any unanchored interval. |
| Failed update | Restore the matching pre-update data/version pair and reconcile subsequent work under the operator procedure. |
| Internet outage | Local clinical, backup, restore and export work continues. Any future external delivery remains queued with visible, owned exceptions. |

Current operational limitations belong in [release readiness](../quality/open-items-g1-g2.md).
Laboratory-specific analyzers, printing, host qualification, volume and recovery targets
remain Q-04, Q-10, Q-18, Q-29, Q-30 and Q-42.
