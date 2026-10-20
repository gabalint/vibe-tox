# Local operations contract

This contract describes the implemented local backup, recovery, audit anchoring, export and host-operation boundaries for REQ-OPS-002/003/006–010/013, NFR-04/12/18/20/23/24/30. Current usage is in the [operator guide](../operations/README.md); unresolved policy and qualification work are in [laboratory policy inputs](../quality/laboratory-policy-inputs.md#operations) and [G1/G2](../quality/open-items-g1-g2.md).

## Ownership and interfaces

`lis-ops` owns installation, backup, restore, runtime settings/history, health, update records and anchor exports. Its `ops`, `ops_runtime` and `ops_anchors` migration modules share the `ops_` prefix. `lis-audit` owns chains and seals; `lis-store` owns snapshots/artifacts; `lis-auth` owns authentication and installation-key use. The engine composes their contracts and the outbound recovery gate.

Current engine signatures and generated bindings in `ui/src/generated/` define the IPC. Native file/folder dialogs run through Rust adapters after command preflight; the UI supplies no filesystem paths or arbitrary filesystem permissions. Commands require engine-side authorization, typed errors and transactional audit. Long file I/O runs outside write transactions. Logs and audit detail fields exclude paths, arbitrary clinical text and key material; attributable reasons use their dedicated field.

## Backup contract

Built-in full encrypted copies use `vtbk1` folder targets: local disk, removable drive or network share. There is no backup sidecar or implemented object-storage client. The [backup and export format](backup-and-export-formats.md) is the canonical byte specification and independent-reader contract.

| Step | Required outcome |
| --- | --- |
| Request/schedule | Persist manual/pre-update requests; determine due available targets from interval and changed audit heads. Only the open application runs jobs. |
| Snapshot | Seal streams, check available space, create a consistent `VACUUM INTO` snapshot and check integrity. A damaged snapshot cannot count as backup. |
| Copy | Encrypt the complete database and manifest; retain content-addressed encrypted artifacts. Missing/damaged source artifacts become explicit problems rather than disappearing from the result. |
| Verify and publish | Read back database and manifest ciphertext digests; decrypt/authenticate the manifest and compare it with the expected bytes. Check artifacts separately, sync and publish the completed set. The database is not decrypted during copy read-back. A partial or cancelled set is never verified. |
| Record | Record each target outcome and the run audit. At least one verified copy permits run success; other target failures and artifact problems remain visible. |
| Reconcile | On startup, unfinished jobs become interrupted. Published sets without a recorded committed copy are quarantined; temporary work cannot silently become a verified set. |

The current interval is **60 minutes**, configurable from 5 to 1,440 minutes. Per target, defaults retain 6 recent, 7 daily and 4 weekly sets, every manual/pre-update set and the newest verified set. Pruning applies only to this installation's eligible scheduled sets. Artifact garbage collection and record-class retention/holds remain unresolved work; backup pruning is not clinical-record disposal policy.

The 15-minute recovery-point target is not established by hourly full copies. [Incremental backup](../planning/backup-incremental-design-brief.md) and [ADR 0014](decisions/0014-incremental-backups.md) remain proposals requiring approval and measurement. Copy duration, write volume and replacement-host restore time require reference-host evidence.

## Keys and recovery kit

One installation key supports sealed authentication secrets and derives the backup wrapping key and key identifier. The OS key store holds the live key; the recovery kit contains the recovery material and target information needed after host loss. Kit export requires fresh normal authentication and explicit protected storage. Anyone holding the plaintext kit can decrypt backups.

Startup distinguishes usable, missing, mismatched and unavailable keys. It does not silently create a replacement key when sealed secrets exist. Missing usable key material prevents backup and requires explicit key recovery. Kit currency and storage confirmation are visible. Key rotation, escrow, hardware-binding qualification and real-host key-store behavior remain release questions (Q-31, G2-15).

## Restore and reconciliation

Restore tests use a scratch directory. Real restore is allowed only on an installation without users, staged and verified before confirmation, and applied on next startup before opening the restored database. Check the key, manifest/set binding, compatible migration ledger, decrypted database digest/integrity, real migrations, chain heads/seals, anchors and every referenced artifact. Record elapsed time and typed findings.

The apply markers progress through ready, applying and applied; invalid staging is failed. Recheck database/artifact digests and absence of users before moving files. Application is resumable after a crash. Replaced database/journal/artifact files move under `ops/replaced/` instead of being deleted. Recording the restore into the restored database must succeed before normal operation can proceed.

Every restore opens a durable recovery reconciliation and raises accession/container number floors by 1,000/5,000 beyond the restored highs. The laboratory may raise them further to cover all labels issued in the lost interval.

| Recovery boundary | Contract |
| --- | --- |
| Outbound actions | Block report release, delivery attempts, delivery-purpose exports and billing handoffs inside their command transactions. Hold label printing. |
| Continuing work | Withdrawal, accessioning, analyzer intake and technical review remain available under their ordinary gates. |
| Closure | A quality-authorized user supplies fresh confirmation, a note and all eleven checklist attestations. The closure is append-only and auditable. |
| Checklist | Replacement-production-computer designation; lost interval; numbering; analyzer resend; reports; orders/QC/reviews; billing; account changes; configuration changes; anchors; waiting labels. |
| Test copy | Keep reconciliation open. Do not let a test copy print or write to production targets. |

Use the [restore runbook](../operations/README.md) for replacement or rebuilt hosts. The application does not restore over an active user installation. A successful synthetic restore test does not establish hardware qualification or laboratory acceptance.

## Audit seals and anchors

Sealing and anchor export run at least every **15 minutes**, independently of the backup interval, and around applicable manual backup/export work. Anchors contain installation/stream identifiers, seal ranges, digests and timestamps, without event content. Every available target can receive anchors; only removable and network-share targets count as off-host.

| Verification case | Expected classification |
| --- | --- |
| Local event at the anchor head matches and the chain is intact | `matched` |
| Anchor extends beyond an explicitly restored head, predates apply, and the restore boundary itself is anchored | `lost_interval`, retained and counted |
| Restore boundary has not itself been anchored | `unverified_restore_boundary`, a warning |
| Local head event is missing without a verified restoration explanation | `local_events_missing` |
| Existing head digest differs | `anchor_mismatch` |
| Seal sequence missing or conflicting | Explicit gap/conflict findings |
| Local seals are newer than exported coverage | Unanchored lag; not itself proof of tampering |

Default off-host lag warning is 30 minutes and incident alert is 24 hours. Missing targets and verification failures stay visible. Lag alone does not stop clinical release. A mutable local hash chain does not prove resistance to privileged tampering; off-host custody and Q-30 remain part of the security boundary.

## Exit export

Complete export requires `export`, fresh normal authentication and a reason. It produces a plaintext folder package with the database, artifacts, schema/migrations, data dictionary, audit anchors and an independent verifier using the Python standard library. Preserve exact issued report bytes and source evidence. The laboratory must protect the export media. This complete installation export does not implement every patient-specific disclosure workflow (REQ-RPT-009/RPT-07).

## Host health and application lifecycle

Health combines bounded OS probes with pure evaluation and versioned laboratory thresholds. Unknown or unavailable evidence produces unknown/warning, never a false green result. State transitions are retained; attention items have an owner and next action.

| Area | Signals and boundary |
| --- | --- |
| Recovery | Backup configured/age/failures; off-host and offline copies; restore-test age; kit currency; open recovery reconciliation. |
| Integrity | Database snapshot integrity, artifact problems, chain verification and anchor lag/findings. |
| Host | Free space, disk encryption, OS support, UPS/time-sync attestations and web-engine changes. Platform detection has known limits requiring real-host checks. |
| Runtime | Listener/folder intake, keep-awake, unacknowledged listening gaps, unclean shutdown, wall-clock changes, outbox failures and version acceptance. |

`lis-ops/src/policy.rs` owns defaults/ranges; `health/` owns evaluation; `host/` owns probes. Daily OS support facts need maintained source evidence and per-release review. macOS support heuristics and unavailable native evidence must stay visibly qualified.

The single desktop process owns listeners and jobs. Keep-awake reflects active intake, with native platform behavior still requiring qualification. Close/quit warnings identify active intake and durable work; an explicit override is audited. Startup applies a pending restore before database open, protects migrations with an intact snapshot, records restore/run/key state, reconciles interrupted work and receipts, then starts listeners/workers. A startup failure displays a typed failure and recovery guidance.

## Update records

The application records a package/version/digest approval, reviewed release information, a retained verified manual or pre-update backup completed within 24 hours, approver and fresh confirmation. Scheduled backups do not qualify. Version history distinguishes approved, unapproved and awaiting-acceptance states; an authorized user records acceptance of the running version.

An unaccepted version raises a banner/work item; the current application does not block release on that condition. Dismissing the banner changes no approval, acceptance or clinical gate. Signed installers, signed update-manifest enforcement, automatic updater/rollback and release identity remain Q-19/Q-34/G2 work. A pre-migration snapshot is a recovery control, not an implemented signed-update channel.

## Verification expectations

| Level | What it establishes | When |
| --- | --- | --- |
| Verified copy | The source snapshot passed integrity checks; written ciphertext digests match; the manifest decrypts and matches expected content; artifact copy checks complete; the set is published. This does not prove the copied database decrypts and opens successfully. | Each backup run |
| Restore test | The set decrypts; the database opens and passes integrity/migration checks; chains, anchors and every referenced artifact verify in scratch storage. | On request; age warnings follow the configured threshold |
| Restore | The restore verification succeeds, then the staged set is applied to an installation without users and recovery reconciliation is opened. | Setup/replacement-host workflow |

| Scenario | Required result |
| --- | --- |
| Restore to fresh folder | Authentication works with recovered key; chains/heads/artifacts verify; reconciliation blocks outbound work; numbering exceeds restored floors. |
| Wrong key, truncated/corrupt database, missing/corrupt artifact, swapped manifest | Explicit corresponding findings; no verified/applicable restore. |
| Modified/recomputed chain, missing tail, substituted older database | Chain or anchor failure according to retained external evidence. An explained restore interval remains distinct. |
| Crash/cancel during backup | Interrupted job; incomplete set is not listed or counted as verified. |
| Offline day and rotating target | Local work persists; missing targets remain visible; return/restart catches up; restore and export remain verifiable. |
| Restore apply interrupted | Resume safely without replacing already verified files incorrectly or losing the required reconciliation gate. |
| Sensitive operation refused | No partial state or key export; authorization/authentication refusal and PHI-free diagnostics follow shared security contracts. |

Use real encryption, file I/O, migrations, digests, chain/anchor verification, restore apply, audit, authentication and outbound gates in synthetic tests. OS facts, native dialogs and inhibitor acquisition may use explicit test doubles. Expected outcomes derive from this contract and the [validation plan](../quality/validation-plan.md).

## Real-host qualification

Required evidence remains: keep-awake on AC and relevant battery behavior; close/quit and OS shutdown; disk-encryption detection; OS/WebView detection; free-space and disk-full behavior; supported local/removable/SMB/NFS/CIFS targets; FAT32 size limits; interrupted copies and Windows rename durability; network database-folder refusal; native dialogs; clock changes; key-store prompts/unlocked sessions; clean replacement-host restore timed against NFR-30; large-database writer/checkpoint and chain-verification costs. The [packaging guide](../development/packaging.md) and G1/G2 register define release ownership.

Implementation reference points: `lis-ops/src/policy.rs`, `backup/copy.rs`, `backup/reconcile.rs`, `restore/apply.rs`, `restore/recovery.rs`, `anchors/`, engine backup/operations adapters and operator runbooks.
