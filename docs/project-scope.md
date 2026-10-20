# Product scope and charter

Status: **confirmed initial scope: US clinical toxicology on one laboratory computer.** Unresolved component, operational and pilot choices remain proposals; owner roles are not yet assigned to named people.

## Mission

Make a free, open-source toxicology LIS that a small laboratory runs on one computer. It should manage the laboratory journey from authorized order to traceable specimen, controlled analysis, reviewed report, delivery, clinical billing handoff and operational follow-up. It should leave the laboratory in control of its data, including a complete export. AI assistance should accelerate development while retaining inspectable decisions, reproducible evidence, and human responsibility for laboratory operation.

"Complete" describes the intended capability map. It does not mean every specialty can share one undifferentiated workflow, or that all capabilities belong in the first release.

## Product decision

The LIS runs locally so the laboratory keeps control of its data. It uses a Rust engine and TypeScript UI in a Tauri 2 desktop window ([ADR 0012](architecture/decisions/0012-rust-tauri-desktop-application.md)).

- **Free and open source.** The whole application, including backup, restore, audit verification and complete export, is free software under GPL-3.0-only and works without any hosted service.
- **One process, no background service.** Analyzers work only while the application is open. It may remain open with nobody signed in; people sign in to act, so each action is attributed to a person.
- **Cross-platform.** The application targets Windows, Linux and macOS, with platform qualification required before a supported release.
- **Local scope.** The current implementation scope covers the LIS application on synthetic data.
- **Optional hosted services.** Clinic ordering, off-site backup, report delivery, EHR exchange and, later, a patient portal are optional future proposals. Each needs a separate scope decision; the LIS works fully without them.

## Product boundaries

The accepted application form and the proposals that depend on it are distinguished in [ADR 0009](architecture/decisions/0009-single-computer-local-lis.md) and [ADR 0010](architecture/decisions/0010-optional-cloud-services.md).

| Boundary | Intended behavior | Status |
| --- | --- | --- |
| One computer, one application | The desktop app owns the database, analyzer listeners, printing and local workflows. It must remain open to receive analyzer messages. | Application form accepted; host qualification remains open (Q-29) |
| Local system of record | External services hold only necessary copies or pending requests. They never change clinical state; the LIS accepts or rejects incoming orders. | Design invariant |
| Offline operation | Local work continues without internet; external delivery waits in visible, reconcilable queues. | Required behavior; see implementation and validation evidence |
| Safety and portability | Local backup, restore, manual delivery and complete export remain available to the laboratory. | Required scope; off-host anchoring details remain Q-30 |
| Future portal scope | Clinic access is limited to the furnishing laboratory's orders and results. | Proposed; laboratory policy and counsel review required (Q-32) |

## Confirmed initial scope

The initial country, testing program and deployment form are **US clinical toxicology on one laboratory computer**. State coverage, patient origins, specimen matrices, assay complexity and the design-partner laboratory still require discovery. Workplace, forensic and postmortem testing are not in scope; they would be separate extensions with their own requirements.

Any expansion beyond toxicology requires a separate scope decision (Q-33).

## Remaining working assumptions

| Topic | Working baseline | What could change it |
| --- | --- | --- |
| Target laboratories | Small US clinical toxicology laboratories whose workflow fits one host computer | Discovery of actual workload, staffing and hardware (Q-02, Q-10) |
| Installation and tenancy | Each laboratory runs its own installation. Any future hosted service would keep a separate database per laboratory ([ADR 0010](architecture/decisions/0010-optional-cloud-services.md)) | Counsel and security review of the service provider's role; tenancy evidence |
| Scale | **Project-owner target, 2026-10-14:** 100–250 patients per day; accession, specimen, execution and analyte counts are separately stated benchmark assumptions ([NFRs](requirements/nonfunctional-requirements.md)). LAN access remains proposed | Partner volumes (Q-10); measured SQLite contention |
| Host computer | A supported Windows 11, macOS or LTS Linux release with full-disk encryption (BitLocker, FileVault or LUKS), an SSD and a UPS, kept powered, awake and signed in while the lab runs analyzers (Q-29) | Partner hardware inventory and OS mix |
| Assays | One lab-selected specimen matrix and a limited menu; representative screening and definitive workflows | Partner lab discovery |
| Clinical responsibility | Qualified laboratory staff approve methods, rules, exceptions, release and each software update | No software design removes this responsibility |
| AI at runtime | No dependency for core operation | Separately scoped, evaluated and governed future feature |

These assumptions bound implementation and laboratory discovery. The country/program part of Q-01 and the product form are resolved. State details and the other [open questions](planning/open-questions.md) remain open.

## Capability map

| Capability | First operational pilot boundary | Complete-product direction |
| --- | --- | --- |
| Installation and operation | Signed installers for Windows, macOS and Linux, lab-approved updates with a pre-upgrade snapshot, local accounts with MFA, encrypted local backup and tested restore to a replacement PC | LAN workstations (Q-28), fleet diagnostics with the lab's consent, guided migration from an existing LIS |
| Identity and clients | Patient matching, provider/client records, explicit correction workflow, clinic-scoped permissions | Clinic self-administration in the portal, delegated administration |
| Orders and catalog | Versioned menu, valid requisitions, individualized order evidence captured as received (classes, diagnosis, signature, standing-order scope), reflex authorization, cancellations; portal orders are requests until accepted | Broad electronic ordering through the portal and EHR interfaces, multilingual collection materials |
| Specimens | Accessioning, barcodes, receipt criteria, containers/aliquots, locations, disposition | Courier/collection tracking, inventory |
| Analytical work | Worklists, batches, supported result imports (ASTM or file), method/QC/calibration provenance, repeat testing | Broader analyzer catalog, automation, deeper vendor data review links |
| Results and reporting | Technical review, authorized release, controlled report templates, corrections and per-channel delivery reconciliation; medication-consistency annotations only where the lab approves and validates them (Q-22) | Configured auto-verification after validation, clinic dashboards and specialized reports |
| Quality management | Method and lot versions, QC holds, deviations, linked SOP/training/maintenance evidence | Full document control, competency management, CAPA, proficiency testing and trend analysis |
| Interoperability | One confirmed inbound order route and one outbound report route, run locally or through an optional hosted service; manual controlled fallback | Client portal, several EHR profiles, FHIR, referral networks |
| Cloud services (optional) | Not required for the first pilot unless the partner selects one; if selected, under a BAA with per-channel evidence | Client portal, managed backup, secure delivery, EHR interface service, later patient portal |
| Clinical billing | Accurate charge events, class-based service derivation from performed work, a payer records-request package and a reconciled billing handoff | Eligibility, claims, denials and payments through validated partners |
| Operations | Audit, access control, backup/restore, retention, downtime/reconciliation, host health checks (disk, OS support, disk encryption, backup age, app-open and listener status) | Capacity planning, analytics, support tooling |
| Workplace/forensic | Data model preserves program boundaries; operational mode disabled | Program-specific custody, split specimens, MRO/case workflows, retention and testimony/export packages |

Detailed acceptance criteria are in [functional requirements](requirements/functional-requirements.md). A linked external system or cloud service can satisfy a capability only when ownership, identifiers, availability, access, retention and reconciliation are demonstrated. "Integration later" is not pilot completion.

## Exclusions from the initial product

The first pilot does not include:

- raw chromatogram interpretation, or instrument firmware or acquisition control
- autonomous medical interpretation, or adherence or diversion conclusions
- determining medical necessity or order scope on a clinician's behalf, or a payer coverage rules engine
- a full EHR, a full insurance billing company, or a replacement for all laboratory QMS obligations
- a cloud-hosted LIS. Optional cloud services would relay, store and deliver; they do not run laboratory workflows
- portal features that give clinics value beyond ordering from, and receiving results of, the furnishing lab (scheduling, e-prescribing, charting, other labs' results)
- a background service, a browser-based UI, or more than one writing host per lab

Method validation and local SOP approval remain laboratory activities supported by evidence and controls. Additional areas require a scoped decision.

## Success measures to establish during discovery

Measure baseline and pilot performance for:

- mislabeled or uncertain identity events
- order-evidence deficiencies found after testing
- payer records-request assembly time and completeness
- receipt-to-release turnaround
- manual transcription
- QC hold handling
- interface and delivery exceptions
- corrected reports
- restore time to a replacement PC

Define denominators and exclusions with the lab. Faster turnaround is acceptable only when required review and quality controls remain demonstrably effective.

No clinical outcome or measured operational performance claim exists yet. The [discovery plan](planning/discovery-plan.md) records how to establish baselines before using them as success claims.

## Decision ownership

- **Project owner:** product scope and priorities.
- **Laboratory director:** clinical use, methods, release policy and acceptance of each software update in the lab.
- **Quality lead:** validation and quality evidence.
- **Privacy/security lead:** data handling, the business-associate inventory and risk decisions.
- **Technical maintainers:** design and software release.
- **Future hosted-service provider:** any separately authorized hosted services, their agreements, subprocessors and continuity.
- **Counsel:** the fraud-and-abuse and business-associate questions (Q-27, Q-32).

One person may hold several roles when appropriate. Roles and conflicts must be recorded before an operational pilot.
