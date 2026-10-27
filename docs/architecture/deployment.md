# Deployment and operational architecture

The local application is implemented for development and evaluation; production deployment is not authorized or qualified. See the [operator guide](../operations/README.md), [packaging guide](../development/packaging.md) and [implementation plan](../planning/implementation-plan.md) for current behavior and verification limits. Hosted services remain proposals.

This document covers the **host computer**, which runs the LIS and keeps the laboratory's system of record ([ADR 0009](decisions/0009-single-computer-local-lis.md)), and the separately proposed **hosted services environment** ([ADR 0010](decisions/0010-optional-cloud-services.md)).

## Deployment profiles

| Profile | Current status | Boundary |
| --- | --- | --- |
| Development and evaluation | Local developer builds with synthetic fixtures and analyzer/file simulators | No patient information or production instrument exports |
| Synthetic demo | Separate DEMO installation, normal sign-in and optional bundled synthetic data | Unsigned development/evaluation build; no shared demo credential or clinical approval from starter data |
| Regulated pilot or production | Not authorized or qualified by the repository | Requires a laboratory, approved scope, real-host evidence, release qualification and explicit authority for real data |
| Hosted-service environment | Proposed in [ADR 0010](decisions/0010-optional-cloud-services.md) | No environment is provisioned and no service is required for local operation |

The [installation guide](../operations/installation-and-first-start.md#choose-a-demo-or-an-empty-laboratory) describes the implemented DEMO separation and reset behavior. Keep demo inputs synthetic; never seed a demo from a laboratory backup. Future external services need separately scoped data flows, identities and credentials.

## Host computer deployment

### Host baseline

Proposed (Q-29, NFR-25; [ADR 0009](decisions/0009-single-computer-local-lis.md#proposal-operating-rules)). Confirm against the design partner's hardware and OS mix before the pilot.

| Item | Windows | macOS | Linux | Why |
| --- | --- | --- | --- | --- |
| Operating system | Windows 11 25H2 or later (Pro, Enterprise or IoT Enterprise), or IoT Enterprise LTSC 2024, on x64. Windows 10 only with paid commercial ESU | A release Apple still patches, on Apple silicon or x64 | An LTS distribution with WebKitGTK 4.1, such as Ubuntu 24.04 LTS, on x64. Supported distributions and whether Linux ships first are Q-29 | Windows 11 24H2 Home/Pro servicing ends 2026-10-13; consumer ESU cannot be used commercially ([ENG-115](../research/sources-engineering.md#single-computer-technology-sources), [§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)). WebKitGTK is the riskiest web engine ([desktop stack research](../research/desktop-application-stack.md#the-cost-of-tauri-three-web-engines-and-linux)) |
| Web engine | WebView2 Evergreen; the configured Windows package includes its offline installer | WKWebView, updated with macOS | WebKitGTK 4.1 from the distribution | Updates outside our releases; the app records the version in the session audit context ([ADR 0012](decisions/0012-rust-tauri-desktop-application.md#consequences)) |
| Disk encryption | BitLocker on (TPM 2.0 and Secure Boot), recovery key held under lab control and off the host, never in a personal Microsoft account. Home edition is not supported | FileVault on, recovery key held by the lab, not only in a personal Apple Account | LUKS full-disk encryption set at install, with a lab-held recovery passphrase | Without a Microsoft account or domain, Windows device encryption "remains unprotected" ([ENG-106](../research/sources-engineering.md#single-computer-technology-sources)) |
| Key storage | Credential Manager / DPAPI, TPM-backed where available | Keychain, Secure Enclave where available | Secret Service; TPM2 where configured | Protects installation secrets and backup keys at rest; any future device certificate needs its own implementation ([desktop stack research](../research/desktop-application-stack.md#per-platform-matters)) |
| Serial ports | COM ports | `/dev/cu.*` | `/dev/ttyUSB*`, `/dev/ttyACM*`; the app's OS user in the `dialout` group | Analyzer connections (Q-18) |

Common to every OS:

| Item | Proposed baseline | Why |
| --- | --- | --- |
| Storage | SSD, enterprise class preferred; free-space floor set by the health check | Consumer drives may "lie about syncing" ([ENG-101](../research/sources-engineering.md#single-computer-technology-sources)) |
| Power | UPS sized for a clean shutdown | Committed work survives power loss only if the drive honors flushes |
| Keep awake and signed in | The OS session stays signed in while the lab runs analyzers; the OS screen may lock. OS sleep is prevented by the app while listeners are active; the lab sets OS updates and reboots outside working hours | Listening stops on OS sign-out, reboot or update ([ADR 0009](decisions/0009-single-computer-local-lis.md#consequences)) |
| Clock | OS time sync on; skew checked by the app (NFR-26) | The host clock is the audit time source |
| Accounts | Per-person OS accounts for administrators; no shared administrator or root password. The OS account that runs the app is a standard (non-administrator) account. Staff use per-person app accounts at the app's sign-in screen ([security](security-and-privacy.md#identity-and-authorization)) | OS administrators and root are the privileged actor |
| Network | Outbound HTTPS to the hosted service endpoints when external services are enabled, and to `github.com` when someone checks for or downloads an update ([ADR 0020](decisions/0020-in-app-updates.md)); no inbound internet ports | The cloud link is outbound only; every laboratory workflow works offline |
| Not supported | ARM64 Windows and Linux (until tested), a database on a network share | SQLite WAL "will not work on a network filesystem" ([ENG-99](../research/sources-engineering.md#single-computer-technology-sources)) |

The onboarding checklist records OS version and support status, ESU status if any, disk-encryption state and recovery-key location, webview version, UPS, power and sleep settings, OS update schedule, and OS audit settings (REG-048, [§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)).

### Application layout and network exposure

- **One application process**, no background service. It runs while open, including at its own sign-in screen with nobody signed in. Only it opens the database ([ADR 0009](decisions/0009-single-computer-local-lis.md)).
- **Single instance.** One running instance per database, enforced by a single-instance lock and a database lock file; a second launch focuses the first ([desktop stack research](../research/desktop-application-stack.md#one-process-what-it-means)).
- **Keep awake and close.** The app holds an OS sleep inhibitor while listeners are active (SetThreadExecutionState, IOKit power assertion, logind inhibitor) and warns before closing with active listeners or pending outbound work. Starting at OS sign-in and minimizing to the tray or menu bar are proposed lab options ([ADR 0009](decisions/0009-single-computer-local-lis.md#proposal-operating-rules)); neither is implemented.
- **Startup reconciliation.** Every start reconciles interrupted imports and expected results and records the listening gap before normal work.
- **Data directory** (database, WAL, artifacts, raw payloads, staging snapshots) in a per-machine location whose OS permissions admit only the app's OS account and administrators.
- **No UI port.** The UI is the app's own window; UI-to-engine calls are in-process Tauri commands. Controls are in [security](security-and-privacy.md#desktop-application-controls) (NFR-31). A LAN workstation listener remains proposed (Q-28).
- **Analyzer listeners** run on serial ports and lab-LAN TCP only. Future MLLP partner listeners require separate implementation and qualification and must stay on the LAN or a VPN.
- **Printing.** Labels use raw ZPL over TCP 9100 or a CUPS raw queue. Windows RAW-spooler support remains unimplemented. Report-window printing and actual printer/stock behavior need native-host qualification.
- **Cloud connector (proposed).** No connector is implemented. A future service link would run inside the application and connect outbound with its device identity ([technology research §6](../research/single-pc-technology.md#6-local-to-cloud-connectivity)).

### Install, update and rollback

The signed release/update flow below remains proposed (REQ-OPS-006, NFR-24, WF-17, Q-34). Current code records version approvals, backup references, first start and acceptance. In builds configured for it, it also checks, downloads, verifies the Minisign signature of, approves and installs a newer published package ([ADR 0020](decisions/0020-in-app-updates.md)). It does not yet verify a signed, expiring manifest (step 2) or any OS publisher signature. CMS surveyor guidance expects the lab to accept a new or revised LIS program before routine use ([§15.4](../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system)).

1. **Install.** The [packaging guide](../development/packaging.md) owns bundle formats, build prerequisites and remaining release work; the current test-bundle configuration has no distribution signing, notarization, updater endpoint or keys. A future signed release must qualify these packages before use. First-run identity and recovery follow [ADR 0019](decisions/0019-security-key-authentication.md) and the [installation guide](../operations/installation-and-first-start.md); the laboratory safeguards the separate installation recovery kit.
2. **Discover.** The app checks a signed, expiring update manifest following TUF concepts: offline root key, expiring timestamp, monotonic versions against rollback and freeze attacks ([ENG-113](../research/sources-engineering.md#single-computer-technology-sources)). It never downloads and installs on its own. The Tauri updater, which requires Minisign signatures on every update artifact, is invoked only after approval ([ENG-132](../research/sources-engineering.md#desktop-application-stack-sources-2026-10-12)). Where the updater does not cover a package format (Linux .deb and .rpm), the lab administrator installs the signed package by hand.
3. **Review.** A lab administrator sees the release package: change summary, risk class, validation evidence, SBOM and signatures (NFR-21). The lab may stage the update on a test copy restored from a snapshot on another machine.
4. **Approve and snapshot.** The approval is recorded with the approver's identity. The app takes a verified pre-upgrade snapshot (`VACUUM INTO`, `PRAGMA integrity_check`, audit-chain head) and ships it to the configured targets before upgrading.
5. **Upgrade.** The app warns about active listeners, stops them, and restarts after the update. Schema migrations are forward-only. Installer rollback covers at most a failed install ([ENG-114](../research/sources-engineering.md#single-computer-technology-sources) for MSI).
6. **Roll back** after a completed install by restoring the pre-upgrade snapshot and reinstalling the prior signed package for that OS. Clinical work committed after the upgrade must be reconciled and re-entered, so rollback is a recorded event, not a routine step. Rehearse it before the pilot.

Security-only patches, including Rust toolchain, crate, Tauri and bundled-library fixes, use an expedited approval channel and are never silent. No service provider may apply updates remotely without the lab's recorded approval (Q-34, proposed). OS web engines (WebView2 Evergreen, WKWebView, WebKitGTK) update outside our releases: clinical logic never runs in the UI, the session audit records the webview version, and the lab runs a short UI smoke check after OS updates ([ADR 0012](decisions/0012-rust-tauri-desktop-application.md#consequences)).

### Health checks

Local health checks and their configured/default thresholds are implemented (REQ-OPS-008, NFR-15); real-host probes and policy suitability require qualification. The [operations contract](operations-contract.md#host-health-and-application-lifecycle) lists the signals (OS support and ESU status, disk encryption, listener gaps, webview changes, free disk, backup and restore-test age (REQ-OPS-007), anchor lag, outbox failures and unclean shutdowns), and the [operator guide](../operations/README.md) explains them. Proposed hosted-service signals are clock skew against server-observed time on each cloud exchange (NFR-26) and per-service cloud queue age and last successful sync (REQ-OPS-009), with external alerts.

The project publishes these checks as the LIS maintenance and function-check instructions that CLIA expects the manufacturer to specify ([§15.4](../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system)).

## Backup, recovery and continuity design

Operational recovery copies are separate from the lab's legal record retention. With one computer, the backup and restore path *is* the CLIA retrieval path and the HIPAA data backup plan ([§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc), [§15.4](../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system)).

### Backup pipeline and targets

The current folder-target pipeline writes full encrypted `vtbk1` copies (REQ-OPS-007, NFR-20, NFR-30). Its steps are in the [operations contract](operations-contract.md#backup-contract), its files in the [formats](backup-and-export-formats.md) and its use in the [operator guide](../operations/backups-and-recovery-kit.md); design evidence is in [technology research §5](../research/single-pc-technology.md#5-backup-and-recovery). `VACUUM INTO` gives a consistent snapshot without stopping work ([ENG-101](../research/sources-engineering.md#single-computer-technology-sources)), and content-addressed artifacts are copied only once. Missed or failed runs alert locally. Object-lock bucket targets, external monitoring and alerts from a hosted backup service remain proposals. On any immutable target, the host's backup credentials can write but cannot delete or shorten retention.

| Target | Managed by | Immutability | Notes |
| --- | --- | --- | --- |
| Second internal disk | Laboratory | None | Fast restore; shares the host's fate in theft, fire or ransomware |
| NAS or file share (backup repository only, never the live database) | Laboratory | Depends on the NAS; snapshots or append-only server | Off the host but often on the same network as ransomware |
| USB rotation with one drive offline | Laboratory | Offline media | Depends on the laboratory's rotation discipline |
| Lab-owned bucket with retention locking (proposed) | Laboratory and its selected storage provider | Write-once if configured and qualified | Requires service-specific retention, deletion, restore and agreement review ([ENG-119](../research/sources-engineering.md#single-computer-technology-sources)) |
| Hosted backup vault | Optional future service provider (REQ-SVC-004) | Object Lock, compliance mode, separate account | Adds monitoring, alerts and scheduled restore verification per the Q-31 option |

At least one copy must be off the host and at least one immutable or offline (NFR-20). The current folder-target implementation does not itself make a target immutable; laboratory media rotation/custody and any server controls need separate evidence.

### Keys and the recovery kit

The OS key store protects the installation data key. The exported installation recovery kit restores encryption access on a replacement computer; it is separate from a person's security-key recovery code and does not authenticate a user. Its content, export confirmation and off-host storage procedure are maintained in [Backups and the recovery kit](../operations/backups-and-recovery-kit.md#the-recovery-kit).

Losing every copy of the installation key makes encrypted backups unusable. Keep the kit and disk-encryption recovery information under laboratory control away from the host. OS key-store behavior, access under the standard application account and replacement-host restoration require qualification. Delegating keys or restore work to an external provider remains proposed under Q-31 and widens that provider's data-handling scope.

### Restore to a replacement computer

Proposed (WF-18, REG-048). The same runbook applies after theft, disk failure, ransomware or a failed rollback.

1. **Declare and fence.** Declare the incident and stop automated delivery. Preserve logs and source payloads. For any future hosted link, revoke the old device certificate before a replacement connects (REQ-SVC-001); no hosted gateway is currently implemented. For ransomware, reinstall the OS or use different hardware; never restore onto the infected installation.
2. **Install.** For a qualified release, install the same verified signed version on a computer meeting the baseline. Current development uses the matching development build. Enable full-disk encryption before restoring data.
3. **Restore.** Enter the recovery kit, choose the newest trustworthy snapshot (from an immutable copy if credentials may be compromised), restore it and the artifact store, and verify chain heads against the off-host anchors.
4. **External links (future).** An accepted hosted service must re-enroll the replacement device and fence the old certificate (WF-19). Current local recovery follows the [restore guide](../operations/restore-and-recovery.md).
5. **Reconcile.** Classify pending, sent, acknowledged and outcome-unknown messages. Compare the restored state with analyzer retention, physical worksheets, cloud-side delivery logs and portal order states, and released reports already held by recipients. Do not replay outboxes blindly or send an older revision after a recipient accepted its correction. Do not reuse an accession or barcode issued during the lost interval.
6. **Record and resume.** Record the unrecoverable interval, affected patients and specimens, and required re-entry, recollection or amended communications under lab procedures. The designated laboratory authority releases the system for use. Record actual RPO and RTO.

A snapshot RPO does not define allowable loss for already acknowledged input or externally issued reports. Retained reports and recipient evidence, raw payloads on backup targets, and any future hosted-service receipts are independent evidence for reconciliation. Agree recovery checkpoints separately for source evidence, clinical commits, issued artifacts, audit and recipient outcomes (NFR-04). If issued content or authority cannot be recovered, record a preservation incident; never manufacture a new release signature from a partner copy.

Transfer time is roughly size divided by bandwidth: 10 GB at 100 Mbit/s is about 15 minutes ([technology research §5](../research/single-pc-technology.md#5-backup-and-recovery); arithmetic, not a measurement). Hardware procurement is outside any restore time the project or a service provider can promise, so the lab should know where a replacement computer comes from.

### Restore exercises

Proposed: before the pilot, after material recovery changes, and at least quarterly in operation, run a restore test (REQ-OPS-007). At least annually, and before the pilot, restore onto a clean replacement computer using only the recovery kit and an immutable copy, with the host's credentials treated as compromised, then complete clinical reconciliation (NFR-20, NFR-30). These frequencies are project proposals, not statutory intervals.

### Host retirement and disk disposal

Replacing or retiring the host, swapping its disk, returning a leased machine and retiring backup media are regulated events: disposal and media re-use are **required** under 164.310(d) ([§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)). The runbook: verify a final backup and restore; revoke the device certificate; clear the disk-encryption keys (BitLocker protectors, FileVault, LUKS key slots) and wipe or destroy the disk; record serial number, method, date and person. Rotated USB media and decommissioned NAS disks follow the same record.

## Host failure behavior

From the [technology research](../research/single-pc-technology.md#failure-behavior); failure scenarios requiring the platform/recovery evidence in ADR 0009 and [ADR 0012](decisions/0012-rust-tauri-desktop-application.md#acceptance-evidence). Hosted-service and signed-update cases apply only when those proposals are implemented.

| Event | Expected behavior | Residual risk |
| --- | --- | --- |
| Power loss mid-transaction | WAL with `synchronous=FULL` keeps committed transactions. Analyzers retransmit unacknowledged frames, because the ACK follows a durable receipt. Outbox leases expire and resume | Drives that misreport flushes. Labels or reports printed for a rolled-back transaction must be reconcilable by ID |
| Disk failure | Restore the latest verified snapshot to a replacement disk or computer | Work since the last off-host snapshot; worksheets and analyzer retention cover re-entry |
| Host loss or theft | Full-disk encryption protects data at rest. Restore from the recovery kit plus an off-host snapshot; revoke any future external-link credential before reconnecting | No off-host copy means data loss; no recovery kit means an unusable backup |
| Ransomware | Live data and writable copies may be encrypted. Reinstall the OS, then restore from an Object Lock copy, offline USB or an append-only server, which the host's credentials cannot delete | Interval since the last immutable copy. Offline media depend on the lab's rotation discipline |
| Privileged local tampering | Hash-chained audit; anchors leave the host on a schedule ([ADR 0008](decisions/0008-tamper-evident-audit-and-immutable-recovery.md)) | An OS administrator or root can alter unanchored events or stop anchoring; anchor lag is alerted |
| Internet or cloud outage | All local workflows continue; cloud services queue; manual delivery available ([overview](overview.md#transaction-and-failure-design)) | Portal turnaround and stale portal copies until reconnect |
| Failed upgrade | Installer rollback where the format supports it; afterwards restore the pre-upgrade snapshot and reinstall the prior signed package | Downtime and re-entry of post-upgrade work |
| Application closed, OS signed out, rebooted or updating | Listeners stop. ASTM senders retransmit unacknowledged messages; watched-folder files wait; the next start reconciles and shows the gap | Analyzers that do not retransmit or retain results; the lab's keep-open procedure |
| Webview update changes UI behavior | Clinical logic and gates stay in the engine; the lab's required UI smoke check checks for display faults; the configured Windows runtime is Evergreen | Display faults between the update and the smoke check |
| OS end of support | Health check warns ahead of the servicing date; the lab upgrades the OS or, for Windows 10, buys ESU | Running unsupported is a documented risk-analysis finding ([§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)) |
| Clock jump | Skew alert; audit records both host time and server-observed time on cloud exchanges | Ordering ambiguity for events in the skew window |

## Hosted services environment

Hosted services are outside current implementation and deployment authority. [ADR 0010](decisions/0010-optional-cloud-services.md) is the canonical proposal for per-laboratory isolation, a PHI-free control plane, outbound device identity, agreements and recovery. The [integration contracts](integrations.md#relay-and-sync-protocol) retain receipt, fencing, correction and reconciliation requirements. Q-09 remains open; no provider or hosted environment has been selected.

A future acceptance review must qualify each service's data flow, US-location/support requirements, agreements, tenant isolation, storage retention locking, key custody and recoverability. Choosing storage branding or a nominal feature is not evidence of those properties. Service objectives remain NFR-28, separate from the laboratory computer's availability.

## Recovery objectives and planning load

Proposed targets live in the [nonfunctional requirements](../requirements/nonfunctional-requirements.md) and take precedence over this summary:

- **NFR-04.** Per-class recovery points and times, with the RTO including restore onto a replacement computer. Acknowledged input and externally issued reports need separately approved loss windows.
- **NFR-30.** Snapshot interval, off-host copy cadence and replacement-computer restore time. The research proposal is a 15-minute snapshot interval ([technology research §5](../research/single-pc-technology.md#5-backup-and-recovery)). The app's current default is hourly full copies; incremental backup remains proposed as a way to meet the 15-minute target ([incremental backup design brief](../planning/backup-incremental-design-brief.md)).
- **72-hour restore.** A project target, not current law. The HIPAA Security Rule NPRM proposes restoring critical systems within 72 hours and backups no more than 48 hours old; it remains proposed (REG-048, [§15.3](../research/regulatory-landscape.md#153-security-rule-on-a-single-pc)).
- **Availability.** The local LIS has no project SLA; it depends on the lab's hardware, power and patching. Cloud service objectives are NFR-28.
- **Workload.** The [scope target](../project-scope.md#remaining-working-assumptions) is 100–250 patients/day, with accession, specimen, execution and analyte counts stated separately. The four-user/LAN hypothesis remains unqualified and does not expand current desktop scope (NFR-29, Q-28). Benchmark the single writer, snapshot duration and backup size with realistic cardinality before committing. Raw chromatography files may dominate storage; the lab supplies sizes and retention.

## Migration, release and exit

**Schema migrations** are forward-only, taken after a verified pre-upgrade snapshot, with a dry run on a restored copy and migration logs. Use expand/contract changes or a tested forward repair. Instrument driver versions, method configuration, report templates and terminology changes need qualification and provenance separate from software updates.

**Migration from an existing LIS** (REQ-OPS-012, P1): map identities, specimen lineage, units, qualifiers, report status, amendments, attachments and retention holds. Run reconciled trial imports and read-only historical retrieval before cutover. Set a cutoff for order ownership and delivery, compare totals and hashes, and keep approved access to the legacy archive until its retention obligations end. The import format must be documented.

**Exit** (REQ-OPS-010, REQ-SVC-005, NFR-12):

- **From the app.** The application exports the database, every artifact, the audit chain, configuration and a documented schema. Because the data already sits on the lab's computer, exit means copying files the lab controls.
- **From future cloud services.** The proposed provider contract requires exporting everything it holds for the lab (orders, clinic directory, delivery and access logs, interface messages), then deletes it after verified transfer and retention approval.
- **If a service provider stops.** The lab keeps operating locally, and the published relay protocol lets another provider or the lab itself run a compatible service (REQ-INT-010).
