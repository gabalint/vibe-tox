# Open items for gates G1 and G2

This is the current register of decisions, implementation limits and qualification
work for the [stage gates](validation-plan.md#stage-gates), which define G1 and G2 and
what synthetic implementation does and does not satisfy. Stable G1/G2 identifiers
remain available for cross-reference. An owner role is not an assigned person, and
an implementation observed in source is not an executed qualification or gate
approval.

## How the gates are read

| Source | Meaning |
| --- | --- |
| Q-01–Q-42 | Stable decisions in [open questions](../planning/open-questions.md) |
| M2-R1–R14 | Laboratory inputs in [order-to-result policy inputs](laboratory-policy-inputs.md#order-to-result) |
| M3-L1–L16 | Laboratory inputs in [reporting policy inputs](laboratory-policy-inputs.md#reporting-and-handoff) |
| M4-L1–L13 | Laboratory inputs in [operations policy inputs](laboratory-policy-inputs.md#operations) |

“Verify” means obtain reviewed evidence for the applicable release and environment.
It does not mean no related automated test exists. “Decide” requires the named
human role; it cannot be satisfied by a proposal written by an agent.

## G1: product implementation entry

| ID | Current item and required disposition | Owner | Canonical basis |
| --- | --- | --- | --- |
| G1-01 | Confirm the small clinical toxicology laboratory workflows that fit one computer. Any expansion beyond toxicology needs an explicit scope decision. | Project, Laboratory | Q-20, Q-33; [scope](../project-scope.md) |
| G1-02 | Select a design-partner laboratory and named operational reviewers; approve its intended-use brief before evaluation. | Project, Laboratory | Q-02; [discovery plan](../planning/discovery-plan.md) |
| G1-03 | Verify the host, analyzer, connection, clinic and label-printer inventory, including stock and network addresses. | Laboratory | Q-04, Q-05, Q-38; [installation](../operations/installation-and-first-start.md) |
| G1-04 | Obtain counsel's portal/interface fraud-and-abuse issue list before implementing or offering those external services. | Counsel | Q-32 |
| G1-05 | Assign maintenance and support responsibilities to named people. | Project | Q-12 |
| G1-06 | Assign reviewers for the bounded implementation and its operational acceptance; implementation authorization does not assign laboratory reviewers. | Project, Engineering, Laboratory | [Implementation scope](../planning/implementation-plan.md#scope); ADR 0013/0018 |
| G1-07 | Decide or explicitly defer payer policy content, upstream review, regulated-function boundaries and partner compliance inputs. | Laboratory, Counsel, Project | Q-21, Q-25, Q-26, Q-27 |
| G1-08 | Confirm applicable states, matrices/menu, release/QC policy, workload/recovery tolerances, optional LAN scope, audit anchoring and key custody. US clinical toxicology and the local desktop product are accepted. | Laboratory, Project | Q-01, Q-03, Q-08, Q-10, Q-28–Q-31 |
| G1-09 | Decide state/accreditation applicability, retention/custody, Part 2 provenance, billing boundary, migration ownership and validity reporting; assess medication consistency and menu governance where enabled. | Laboratory, Counsel | Q-13–Q-17, Q-22–Q-24 |
| G1-10 | Local Rust/Tauri build direction and the desktop architecture are accepted. Disposition remaining component and operational proposals individually. | Engineering, Project | Q-06, Q-36; [ADR statuses](../architecture/decisions/README.md) |
| G1-11 | Qualify supported OS versions and Linux distributions and decide the first release's supported combinations. The project owner requires Windows, Linux and macOS. | Engineering, Laboratory | Q-29; [OS test matrix](../governance/engineering-standards.md#os-test-matrix) |
| G1-12 | Obtain real-host evidence for sign-in-screen ASTM over serial/TCP, raw label printing, power loss and large grids in the native web engines. | Engineering | G2-20–G2-27; [ADR 0012](../architecture/decisions/0012-rust-tauri-desktop-application.md) |
| G1-13 | Qualify generated PDF accessibility, including the applicable PDF/UA check and manual reading/assistive-technology evidence. | Engineering | ADR 0012; VAL-13/32 |
| G1-14 | Approve order completeness, validity, rounding, analyzer corrections, performed-not-ordered handling, QC, repeat/reflex, material and collection-time policies. | Laboratory | M2-R1–R14 |
| G1-15 | Approve report grouping, partial/final/corrected issue, release authentication, validity/material holds, destinations, withdrawal, impact review and billing policies. | Laboratory | M3-L1–L16 |
| G1-16 | Approve backup/export authority, schedule/retention, recovery kit custody, restore tests, anchor targets, post-restore reconciliation, updates, session limits and host restore procedures. | Laboratory | M4-L1–L13 |
| G1-17 | Accept or reduce the residual risk of one person holding account-administration and clinical authority; assign review of role-change and credential-reset events. The configurable one-person policy does not itself establish laboratory acceptance. | Laboratory | Q-37; [ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md) |
| G1-18 | Approve identifier normalization for each assigning authority. | Laboratory | [Identity requirements](../requirements/functional-requirements.md); [domain model](../architecture/domain-model.md) |
| G1-19 | Accept analyzer TCP peer-address spoofing risk or specify a qualified gateway/VPN control. | Laboratory, Engineering | [Security architecture](../architecture/security-and-privacy.md); [integration contracts](../architecture/integrations.md) |
| G1-20 | Decide remaining slice boundaries: downtime-issued reports, oral orders/AOE, standing-order renewal, automated reflex/acceptance rules, dilution arithmetic, wrong-patient reassociation, multi-accession reports, competency/location policy, retention schedules and OS-start behavior. File intake and configurable separation of duties are implemented; do not treat their presence as qualification of the other functions. | Laboratory, Project, Engineering | [Implementation scope](../planning/implementation-plan.md#scope); M2/M3/M4 scope cuts; ADR 0015 |
| G1-21 | Approve clock-correction and recovery procedures. A backward clock step can block TOTP sign-in until the replay window catches up. | Laboratory, Engineering | [Authentication implementation](../../crates/lis-auth/src); NFR-19/26 |

## G2: signed-installer synthetic demonstration

### Signing, packaging and updates

| ID | Current item and required disposition | Owner | Canonical basis |
| --- | --- | --- | --- |
| G2-01 | Decide legal publisher, signing identity and stable application identifier before distribution and key-store/data-path migration. | Project, Engineering | Q-19; [packaging](../development/packaging.md#release-readiness) |
| G2-02 | Configure and qualify Windows signing, macOS signing/notarization and the selected Linux signing route on clean hosts. Developer bundles do not close this item. | Engineering | [Configured bundles](../development/packaging.md#configured-bundles); VAL-37 |
| G2-03 | Development release channels are set ([branches and releases](../development/releases.md)). Decide supported-release channels, support windows and update policy; implement and qualify signature/expiry/downgrade rejection, lab approval and expedited security updates. The in-app updater ([ADR 0020](../architecture/decisions/0020-in-app-updates.md), accepted 2026-10-27) verifies Minisign package signatures bound to their version, installs only newer versions and requires the recorded approval and verified pre-update backup. It is unconfigured until the signing secret and public key are set. A signed, expiring manifest, freeze detection and per-OS installation qualification are still missing. | Project, Engineering | Q-34; [packaging](../development/packaging.md); NFR-24 |
| G2-04 | Qualify package rollback with snapshot restore and reconciliation of post-upgrade work on every supported OS. | Engineering | VAL-37; [update-record contract](../architecture/operations-contract.md#update-records) |
| G2-05 | Verify installer handling of unsupported OS versions/editions against the approved platform baseline. | Engineering | VAL-48; NFR-25; Q-29 |
| G2-06 | Decide whether any RPM-family distribution is supported before adding or qualifying RPM packaging. Existing Linux bundles are Debian and AppImage. | Engineering | [Configured bundles](../development/packaging.md#configured-bundles) |
| G2-07 | GPL-3.0-only is adopted. Since 2026-10-20, unsigned development builds are conveyed on GitHub Releases with their tagged source by project-owner instruction, but the installers carry no third-party license/notice inventory yet. Generate and ship that inventory with the published builds. Complete ownership administration, compatibility review and corresponding-source verification before a supported release. | Project, Counsel | Q-11; [open-source strategy](../governance/open-source-strategy.md); [packaging](../development/packaging.md#release-readiness) |
| G2-08 | Prepare and review the release validation package, including approved risk classes, supported configuration and deviations. | Engineering, Laboratory | [Release package](validation-plan.md#release-validation-package-for-laboratory-acceptance) |

### Security and operational controls

| ID | Current item and required disposition | Owner | Canonical basis |
| --- | --- | --- | --- |
| G2-09 | Native FIDO2 authentication is implemented, with a password/TOTP alternative, optional touch-only own sign-in/unlock and a personal same-session unlock PIN. Role-wide FIDO2 enforcement for release and administration and the sealed break-glass account (REQ-IDN-004, NFR-19) are not implemented. Qualify real keys/firmware and normal-user OS access, fresh verified sensitive actions, no failed-PIN fallback, policy races, enrollment/migration, replacement/revocation and restore recovery. Laboratory risk acceptance remains open. Replace the pinned OpenSSL source with a maintained integration carrying applicable fixes before production, then repeat advisory, native-key and package qualification. Scoped applicability review is not a waiver. | Engineering, Laboratory | Q-42; [ADR 0019](../architecture/decisions/0019-security-key-authentication.md); [native cryptography qualification](../development/packaging.md#native-cryptography-qualification); VAL-53 |
| G2-10 | Explicit restrictive Windows recovery-kit ACL remains an implementation gap: secret files inherit the chosen folder's ACL. Qualify the chosen fix or accountable environmental control. | Engineering, Laboratory | [Durable file handling](../../crates/lis-ops/src/backup/durable.rs); [recovery guide](../operations/backups-and-recovery-kit.md) |
| G2-11 | Resolve and qualify Windows directory/rename durability under power loss; read-back verification alone does not establish durable directory metadata. | Engineering | [Durable file handling](../../crates/lis-ops/src/backup/durable.rs); VAL-38/39 |
| G2-12 | Export intent is implemented before report/billing bytes are published, with residual-file evidence on failed cleanup. Retain crash/failure coverage in release qualification. | Engineering | [Export implementation](../../crates/lis-engine/src/report_rendering/export_file.rs) |
| G2-13 | Detailed work-queue item reads are access-audited. Summary reads use the ordinary read path; review their count/metadata sensitivity and decide any remaining access-audit requirement. | Engineering, Laboratory | [Work queues](../../crates/lis-engine/src/commands/work_queues/mod.rs); [security/audit standard](../governance/security-audit-logging.md) |
| G2-14 | Verify actor-reference integrity in every module and revocation/capability races for all commands. A transaction's check of a previously resolved principal is not evidence of a universal current-authority recheck. | Engineering | [Command context](../../crates/lis-store/src/context.rs); [engine pipeline](../../crates/lis-engine/src/pipeline.rs); VAL-08/21 |
| G2-15 | Decide installation-key rotation and its effects on backups, TOTP seeds and recovery. | Engineering, Laboratory | [backup contract](../architecture/operations-contract.md#backup-contract); Q-31 |
| G2-16 | Obtain privacy review of digest-only anchors, their metadata and disclosure boundaries. | Laboratory | [Audit/recovery architecture](../architecture/decisions/0008-tamper-evident-audit-and-immutable-recovery.md); VAL-40 |
| G2-17 | Review the current application's threat model and resolve or explicitly accept its applicable findings for the release. | Engineering, Laboratory | [Security architecture](../architecture/security-and-privacy.md); [implementation milestones](../planning/implementation-plan.md#milestones) |
| G2-18 | Database-damage alerts for failed snapshot integrity are implemented; preserve failure/recovery coverage in release qualification. | Engineering | [Backup health rules](../../crates/lis-ops/src/health/backup_rules.rs); [health tests](../../crates/lis-ops/src/health/tests.rs) |
| G2-19 | Consecutive backup failures produce a warning and an alert at three failures; qualify target-specific failures and recovery under the lab's approved policy. | Engineering, Laboratory | [Backup health rules](../../crates/lis-ops/src/health/backup_rules.rs); [backup guide](../operations/backups-and-recovery-kit.md) |

### Real-host verification on Windows, macOS and Linux

| ID | Required evidence | Owner | Canonical basis |
| --- | --- | --- | --- |
| G2-20 | Keep-awake, close/quit, sign-out, reboot, sleep, listener gaps and second-instance handling. | Engineering | VAL-56; [real-host checks](../development/packaging.md#m5-checks-on-real-hosts) |
| G2-21 | Disk encryption, OS support, disk space, clock and power assumptions; OS key-store access including Linux Secret Service while at the LIS sign-in screen. | Engineering | VAL-48; [real-host checks](../development/packaging.md#m5-checks-on-real-hosts) |
| G2-22 | Applicable SMB/NFS/CIFS/removable targets, FAT32 limits, interrupted copy, actual disk full and network-folder refusal. | Engineering | [real-host qualification](../architecture/operations-contract.md#real-host-qualification) |
| G2-23 | Timed clean-machine restore using only the off-host copy and recovery kit, including OS key-store writes, restored artifact/audit verification and relaunch. | Engineering, Laboratory | VAL-39; NFR-30; [recovery guide](../operations/backups-and-recovery-kit.md) |
| G2-24 | Physical power loss during import, release, dispatch, snapshot and printing; native/USB serial; raw label printing through the OS spooler/CUPS; unreachable printer behavior. | Engineering | VAL-38/50/51 |
| G2-25 | Native dialogs, cancellation, threading and unblocked OS shutdown on each supported host. | Engineering | [Real-host checks](../development/packaging.md#m5-checks-on-real-hosts) |
| G2-26 | Complete native journeys, large grids, exact decimal display, PDF behavior, keyboard/focus/screen-reader access and update smoke checks in WebView2, WKWebView and WebKitGTK. Mocked IPC and a native startup smoke check cannot close complete workflow qualification. | Engineering, Laboratory | VAL-02/13/57; NFR-09/33 |
| G2-27 | Writer wait, snapshot time and native/UI response timing on each reference host using lab-approved daily, peak and accumulated-history workloads. Historical billing/delivery throughput and all-platform capacity remain unqualified. | Engineering, Laboratory | Q-10; NFR-05/29/33; [capacity tests](../../crates/lis-engine/tests/perf/remediation_capacity.rs) |
| G2-28 | Bounded review/work-queue paging and continuation are implemented. Verify sparse/history-dependent scans remain complete and responsive under the target workload; an incomplete count is not zero. This item feeds G2-27. | Engineering | [Review paging regression](../../crates/lis-engine/tests/scenarios/remediation/review_paging.rs); [queue implementation](../../crates/lis-engine/src/commands/work_queues/mod.rs) |
| G2-29 | Audit-chain/anchor verification, full-copy backup I/O and impact-scan cost on reference hardware. | Engineering | NFR-05/18/29; [backup/export formats](../architecture/backup-and-export-formats.md) |
| G2-30 | Independent review and execution of the backup-format decryptor/verifier against complete and damaged packages. | Engineering | [Backup/export formats](../architecture/backup-and-export-formats.md) |
| G2-31 | Decide and qualify app-driven report printing, exact issued-artifact handling and intact-arrival checks. | Laboratory, Engineering | Q-18; REG-047; VAL-52 |

### Policy and remaining coverage

| ID | Current item and required disposition | Owner | Canonical basis |
| --- | --- | --- | --- |
| G2-32 | Approve platform, backup-key, anchoring, printing and health-threshold pass criteria; apply LAN/cloud criteria only if those capabilities enter scope. | Laboratory, Project | Q-18, Q-28–Q-32; [scenario conditions](validation-plan.md#single-computer-and-cloud-service-scenarios) |
| G2-33 | Accept or revise proposed skew, writer-wait, recovery and interaction targets and the retained-history workload. | Laboratory, Project | [Nonfunctional requirements](../requirements/nonfunctional-requirements.md); Q-10 |
| G2-34 | Run-directory pagination and a loaded count are implemented. Qualify long analyte identifiers in the changed-QC grid and detail panel, and the remaining native UI journeys; source review does not establish visual or assistive-technology suitability. | Engineering | [Runs page](../../ui/src/features/analytics/runs/RunsPage.tsx); [changed QC](../../ui/src/features/analytics/qc/ConflictsTab.tsx); G2-26/27 |
| G2-35 | Complete requirement-level coverage for cancellation/reflex races, per-analyte repeat selection, released-result re-export, backdated configuration, PHI log canaries and exit-export reimport. Related tests exist; identify and run exact applicable protocols before closing the larger scenarios. | Engineering | VAL-15/17/21/27/47/54; [engine scenarios](../../crates/lis-engine/tests/scenarios) |
| G2-36 | Decide and implement applicable diagnostic-log/support-bundle retention and artifact garbage collection with record-class holds and recovery effects. | Engineering, Laboratory | NFR-32; REQ-OPS-003; [backup contract](../architecture/operations-contract.md#backup-contract) |
| G2-37 | Decide whether lab-owned write-once backup/anchor targets enter the release; qualify any selected external backup tool separately. Offline copies do not establish immutability or cloud-service readiness. | Engineering, Laboratory | Q-30/31; VAL-39/40 |

## After G2: shadow, pilot and production (G3–G5)

| Item | Gate or scope | Owner |
| --- | --- | --- |
| Resolve [DM-07 source custody, RPT-06 urgency/delay communication and RPT-07 patient/proxy disclosure](scope-decisions.md), or approve and qualify their explicit external-process alternatives. | Before the corresponding clinical workflow is relied upon | Project, Laboratory |
| Authorize real-data handling; qualify the host and off-host key/kit custody; approve downtime procedures and rehearse recovery at the laboratory. | G3 | Laboratory |
| Accept the installed release through the laboratory's release package, including training, configured methods/interfaces/reports and SOPs. | G3/G4 | Laboratory |
| Complete laboratory validation for the configured intended use; synthetic tests do not establish clinical suitability. | G4 | Laboratory |
| Approve optional cloud hosting, residency, business-associate arrangements and patient-portal scope before those services start. | GC-1/GC-2; later portal scope | Project, Counsel, Laboratory |
| Qualify distribution rights, signing, release channel, support window and security-response roles for supported software releases. | G5/publication | Project |
