# ADR 0008: Tamper-evident audit chain and immutable recovery copies

Status: proposed, 2026-10-12. Decision owners: security/privacy lead, technical maintainer and the service provider for any optional hosted anchor store and backup vault. The laboratory approves the residual privileged-access risk.

## Context

Application-level append-only audit does not resist someone who can edit the database directly ([security](../security-and-privacy.md#audit-secrets-and-retention)).

On one lab computer, that someone is the **lab's OS administrator or root user**, on Windows, macOS or Linux ([ADR 0009](0009-single-computer-local-lis.md)). Anyone with administrator rights on the host can:

- read, copy or edit the SQLite file;
- recompute a hash chain;
- close or kill the application, replace its binaries, change the clock;
- disable backups and anchoring.

There is no separate database or cloud administrator layer between the lab and its data. Small labs often have one or two people with these rights, sometimes an outside IT contractor.

The implemented application provides a hash chain, seals, anchor export and encrypted full backups to folder targets. Independent off-host custody and immutable target enforcement remain qualification/proposal boundaries. A hash chain alone cannot resist whole-chain replacement by an actor who controls the host.

Evidence for storage-lock semantics is retained in [ENG-119](../../research/sources-engineering.md#single-computer-technology-sources) and [PLAT-13](../../research/sources-engineering.md). [ENG-49](../../research/sources-engineering.md) records the audit-design basis; regulatory backup proposals remain separately identified in the [regulatory landscape](../../research/regulatory-landscape.md#153-security-rule-on-a-single-pc). No storage provider or backup sidecar is selected by this decision.

## Proposal

1. **Chain audit records.** Clinical audit events, release manifests (report revision ID, artifact digest, recipients) and interface receipts are appended within their committing `BEGIN IMMEDIATE` transaction. Each event carries a per-stream sequence number and the hash of its predecessor. Hash inputs use a canonical serialization and a versioned algorithm identifier.
2. **Seal and anchor segments off the host.** An in-process worker seals the chain segment at a fixed interval, proposed no longer than the approved recovery point (NFR-04, NFR-30).
   - **Anchor content.** Only digests and metadata: stream ID, sequence range, segment digest, head hash, seal time and algorithm/serialization version. No event content, record identifiers or patient data, so anchor storage stays outside PHI scope, subject to privacy-officer confirmation.
   - **Anchor destinations.** Anchors must leave the host, because anything on the host is within the administrator's reach. The lab chooses one or more destinations:

     | Destination | Path | Notes |
     | --- | --- | --- |
     | Hosted anchor store (optional service) | Outbound link with the device certificate | Compliance-mode write-once storage in an account separate from the service runtime |
     | Lab-owned write-once bucket | Direct from the host | The host's credentials can write but not delete or shorten retention. The lab holds the storage account; its administrator must not also be the host's sole administrator if the separation is to mean anything |
     | Offline media | Export file to rotated media | Evidence quality depends on the lab's rotation and custody discipline |

     Q-30 asks whether every install includes anchoring without any hosted service; the proposal is yes, so this safety control never depends on one (INV-16).
   - **Verification.** Verification compares live chains against the anchors and alerts on gaps, forks, reordering or lag. Restore verifies chain heads against anchors before the lab resumes work.
   - **Full segment content.** Audit events and release manifests do contain identifiers and PHI. Their full content is preserved through the immutable recovery copies in item 5.
3. **Anchor lag and outage behavior.** Clinical operation, including release, continues when no anchor destination is reachable. Sealed digests queue durably in the database and are anchored in order once a destination returns.
   - **Lag alert.** Anchor lag is a host health check (REQ-OPS-008). Exceeding the NFR-18 target raises an alert. Exceeding a second, lab-approved threshold opens an incident. With an optional external notification service the alert also reaches the lab's contacts through the service provider, without PHI.
   - **Release.** Anchor lag never blocks release unless approved laboratory policy explicitly says so.
   - **Residual risk.** Unanchored intervals are recorded so the residual-risk statement reflects the actual exposure window. An administrator can stop anchoring; the lag alert is what makes that visible.
4. **Collect host activity separately (optional).** OS file-access auditing on the data directory (Windows object-access auditing, or the macOS or Linux audit subsystem), with events forwarded off the host, can record direct file access outside the application. This is an inference to test. The same administrator can disable it, so it supplements the chain and anchors and does not replace them.
5. **Make recovery copies immutable.** The backup pipeline ([deployment](../deployment.md#backup-recovery-and-continuity-design)) supplies the copies:
   - **Pipeline.** `VACUUM INTO` snapshots verified with `PRAGMA integrity_check` and the audit-chain head, plus content-addressed artifacts. The local application uses built-in encrypted `vtbk1` copies to folder targets ([formats](../backup-and-export-formats.md)); write-once bucket integration remains proposed.
   - **Targets.** At least one target is write-once or offline: a lab-owned Object Lock bucket, rotated offline media, or an optional external Object Lock vault (NFR-20, REQ-OPS-007).
   - **Credentials.** The host's credentials cannot delete or shorten retention on these copies.
   - **Ransomware.** After ransomware or suspected administrator compromise, restore starts from these copies onto a rebuilt or replacement computer.
6. **No external timestamping for the pilot.** RFC 3161-style external timestamping can be added later if a legal or evidence need requires proof of time independent of the host clock and the storage provider.

## Alternatives and consequences

| Option | Assessment |
| --- | --- |
| Application append-only tables only | Simple, but the OS administrator can rewrite history undetectably. Insufficient for the stated residual-risk goal. |
| Hash chain on the host with no external anchor | Detects naive edits but not wholesale chain recomputation by an administrator. |
| **Chain plus off-host write-once anchor (proposed)** | Detects modification, deletion and reordering up to the anchor lag. Adds a sealing worker, verification jobs, serialization versioning and a storage destination. |
| SQLite encryption with a key on the host | Protects copied files, not a live administrator ([ENG-103](../../research/sources-engineering.md#single-computer-technology-sources)). Not a tamper-evidence control. |
| Ledger database or blockchain | Adds a new runtime dependency and operating burden without a demonstrated need. |

Consequences:

- **Residual window.** Tampering inside the unanchored window remains possible and must be stated in the residual-risk record. So does tampering by someone who controls both the host and the anchor destination. Every OS administrator (and anyone with root) on the host is named in that record.
- **Separation strength varies by destination.** A lab-owned bucket administered by the same person as the host, or offline media without custody, gives weaker separation than a hosted store run by a separate provider. The residual-risk statement records which destination the lab chose.
- **Retention.** Compliance-mode retention cannot be shortened. Retention periods need approval from the records owner before enabling. Test environments use governance mode or short periods.
- **PHI scope.** Digest-only anchors keep the anchor store outside PHI scope. The immutable recovery copies hold PHI as ciphertext. A provider storing them is a business associate even without the key ([§15.1](../../research/regulatory-landscape.md#151-hipaa-role-of-each-hosted-service)): the service provider for a hosted vault, or the lab's own storage provider for a lab-owned bucket.
- **Anchor outage.** An outage widens the residual window but does not stop clinical work. Monitoring must make the widened window visible.
- **Provider support.** A future hosted anchor store or vault requires demonstrated retention locking, credential separation and deletion/restore tests for the selected service and configuration.

## Acceptance evidence and revisit

Before acceptance, the following must be demonstrated on synthetic data:

- detection of altered, deleted, inserted and reordered audit rows, including chain recomputation by an administrator editing the SQLite file
- detection of a replaced report artifact file
- detection when an anchor is missing
- anchor-lag alerting and incident escalation during a simulated outage and after anchoring is deliberately stopped, with release continuing
- the same detection with each destination: hosted store, lab-owned bucket and offline media
- inspection showing that anchors contain no PHI or record identifiers
- restore from an immutable copy onto a replacement computer, with the host's credentials assumed compromised
- a documented residual-privilege statement naming the host's administrators and the chosen anchor destination

Revisit if:

- the HIPAA Security Rule is finalized with different backup or audit requirements;
- a lab requires independent timestamping;
- Q-30 decides that some installs can anchor only through a hosted service;
- the chosen storage lacks compliance-mode retention.
