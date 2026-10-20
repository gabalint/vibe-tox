# Laboratory discovery plan

Status: **proposed laboratory discovery; no engagement conducted**. Discovery targets physician-office and small US clinical toxicology laboratories that fit one host computer ([scope](../project-scope.md#product-decision)). The first target workflow and partner are open under Q-20, Q-33 and Q-02. Because the application must stay open for analyzers to work ([ADR 0009](../architecture/decisions/0009-single-computer-local-lis.md)), discovery also records each lab's OS mix and whether its host can stay signed in and awake. No laboratory interviews, vendor outreach or counsel engagement have been conducted; this plan does not authorize outreach or handling real clinical records.

## Intended-use brief

Before accepting a laboratory pilot scope, produce one owner-reviewed brief with:

- the legal laboratory entity, its sites and whether it is a physician-office lab (POL) of a referring practice or an independent lab;
- patient and collection footprint, and the states involved;
- clinical use and exclusions, specimen matrices and menu, and CLIA certificate type and complexity per test;
- order authority, ordering clinics and how their orders arrive;
- the host computer, workstations, analyzers and connection types;
- report recipients and delivery channels;
- the incumbent LIS and other systems, and the human owners.

Record whether the lab is replacing an LIS, opening a service, extending an existing system or considering referral testing; these have different migration and operating costs.

| Discovery work | Evidence to obtain in an authorized engagement | Decision affected / owner |
| --- | --- | --- |
| Laboratory operational needs | Laboratory owner, supervisor, bench and quality users, recurring failure/delay, current workaround, expected workflow benefit; record observed facts separately from requested features | Q-02/20/33; project owner + operations |
| Intended use and clinical boundary | Approved brief, methods and complexity, direct definitive/reflex/repeat cases, reporting and urgent-notification policy, state/program applicability | Q-01/03/08/13; director + quality |
| Host computer and IT support | Workstations and concurrent users; OS edition, version and support status; disk encryption, TPM where Windows, disk type and UPS; power, sleep, screen-lock and OS-update settings during analyzer hours; update and recovery-key responsibility; outside IT support. See [host questions](#questions-for-office-and-small-labs) | Q-28/29/31; technical lead + practice manager |
| Analyzers and connections | Each analyzer and quantitation package with version, connection type and adapters, co-located vendor software, and permitted synthetic format descriptions | Q-04/25; technical lead + interface owner |
| Existing records and exit | Existing software version and export formats, source ownership, historical reports and retention responsibilities | Q-17; operations + quality |
| Backup and recovery practice | Backup target, frequency and checks; last restore test; offsite or immutable copy; recovery time for a failed PC; downtime procedure | Q-10/31; operations + quality |
| Orders and ordering clinics | Order channel mix; ordering clinics, their EHRs and relationship to the practice; clinic requisition and label practice | Q-05/32; operations + interface owner |
| Delivery and interfaces | Report channels per clinic; fax-number verification; who pays EHR-vendor interface fees and onboarding time; interface engine or connectivity vendor in use | Q-05/32; client services + interface owner |
| Payer, billing and compliance context | Payer mix and Medicare contractor/LCDs. Order-evidence practice: individualized classes, standing orders and signatures. RCM vendor and contract boundary. Denial and audit history: ADR/TPE/UPIC requests and turnaround. Compliance officer, client types and arrangements. Same-day presumptive-plus-definitive practice | Q-16/21/27; revenue-cycle lead + compliance + counsel |
| Counsel review (separately authorized) | Counsel's view of the per-lab portal boundary, who may pay clinic EHR-vendor fees, EKRA-covered clients, and Stark in-office ancillary questions for a POL taking outside clinics ([regulatory landscape §15.5](../research/regulatory-landscape.md#155-fraud-and-abuse-laboratory-furnished-clinic-portals)) | Q-27/32; counsel + compliance officer |
| Workload and measurable value | Accessions per month and peak day; manual handling time, exception ageing, turnaround, interface and correction effort; agreed denominators and observation window | Q-10/20; operations + product |

Record each item as requested, observed, owner-approved or unresolved, with date and evidence location. Empty fields are discovery gaps. A vendor demonstration or a staff preference alone does not establish demand, a validated method or operational acceptance.

## Design-partner selection criteria

Apply these criteria before investing discovery effort in a candidate laboratory. They are proposals for the project owner; they do not authorize outreach.

| Criterion | Why it matters | Evidence to request |
| --- | --- | --- |
| Fits one host computer and the accepted target of 100–250 patients/day | Measure accessions, specimens, executions, bursts and history separately; capacity is not qualified and LAN workstations remain proposed | Workstation count, user list, CMS-116 volume or monthly accessions (Q-10, Q-28) |
| Performs in-house toxicology testing on a bounded menu with an accountable director and quality lead | The pilot needs real method, QC and review policies to observe | CLIA certificate type and specialties, menu and method list, named reviewers (Q-02/03) |
| Analyzers reachable by serial, TCP or file export, with permitted access to synthetic or de-identified format descriptions | Without format access, no driver or provenance design can be validated | Analyzer and software inventory, written permission and sample format descriptions (Q-04/25) |
| Has, or will buy, a host computer on the supported baseline, and has someone to maintain it | An unsupported or unpatched host is a risk-analysis finding before any LIS question | OS, edition and version, disk encryption, SSD, UPS, whether it can stay signed in and awake, IT support arrangement (Q-29) |
| Has outside ordering clinics, if the pilot is to test the client portal | External-clinic workflows need a partner that actually uses them | Clinic count, EHRs, order channels (Q-05) |
| Counsel-reviewed integrity profile | Drug-testing enforcement concentrates on order design and client arrangements. A partner's practices can create reputational and legal exposure for the project | Compliance program summary, payer audit history, client types, and counsel's go/no-go. The project does not judge legality (Q-27) |
| Assigned time for validation, update acceptance and support | The lab must accept every release before use; an open-source pilot without assigned validation and support cannot progress to laboratory use | Named owners, available time, support model, who signs update acceptance (Q-12, Q-34) |
| Willingness to share non-sensitive results publicly | Open-source credibility depends on publishable synthetic case packs and lessons | Agreement on what synthetic artifacts and findings may be published (Q-11/19) |


## Lab discovery case pack

In a later authorized engagement, have an accessioner, bench analyst, technical reviewer, authorized releaser, quality lead, practice manager or IT support person, billing staff and a client/report recipient walk through the same fully synthetic case. Record volumes, handoffs, delays, exceptions, systems of record, actual authorizations, physical labels and workstations, and what happens when the lab computer is off or offline. Do not gather patient data into research tools. Any necessary real-data observation requires the authorized handling environment first.

Request permitted versions or descriptions of specimen acceptance/rejection SOPs, assay/method/QC validation records, report and amendment templates, critical-result procedure, current role matrix, downtime and backup records, the last LIS update acceptance record, retention schedule, analyzer/version inventory, clinic interface specifications, billing handoff and QMS links. Access to licensed or confidential documents must use the authorized environment; the repository stores permissible summaries and evidence references.

## Questions for office and small labs

**Host computer and IT**

- How many computers touch the LIS today, and how many people use it at the same time? Is one computer the server?
- Which operating system runs on each (Windows, macOS or Linux), with which edition and version? Is full-disk encryption on (BitLocker, FileVault or LUKS), and where is the recovery key? Is there a UPS?
- Can the LIS computer stay powered on, awake and signed in to the OS, with the LIS app open, whenever analyzers send results, including nights and weekends? Who may sign out or restart it, and when do OS updates restart it?
- Who installs OS updates and LIS updates? Is there an outside IT provider or MSP?
- How is an LIS update accepted today, and who signs it?

**Analyzers**

- Which analyzers and quantitation packages send results to the LIS, and how: serial cable, network, a file in a folder, or typing?
- Does any analyzer's software run on the LIS computer? Does any route need someone signed in to the OS to work?
- If the LIS computer is off or the app is closed, does each analyzer keep and retransmit unsent results, and for how long? (RISK-56)

**Existing records and migration**

- Which source system holds existing records, and which version and export formats must migration support?
- Can the lab export its data today, in what format, and who must help?

**Backup and recovery**

- Where do backups go, how often, and who checks them? When was a restore last tested, and on what machine?
- If the LIS computer died this morning, what would happen and how long would it take?

**Orders, clinics and delivery**

- What share of orders arrives by fax, paper, a portal or an EHR interface?
- How many clinics order, which EHRs do they use, and are they part of the same practice?
- Who paid each clinic's EHR-vendor interface fee, and how long did onboarding take?
- How are results delivered to each clinic, and how is a new fax number checked?

## Questions that change design

- Can orders be accepted before all demographic/authorization data are known, and what holds persist?
- How are containers, aliquots, splits, repeats, dilutions and send-outs identified and reconciled physically?
- Which analytes are qualitative, quantitative or both, and how are invalid/censored results represented?
- What instrument export fields establish sample position, method version, unit, flags, calibration and QC linkage?
- Which exceptions require independent review, which are prohibited, and how are retrospective failures handled?
- Who can release to which recipients, how are critical/delayed/corrected reports acknowledged, and which access rules apply?
- What work continues when the lab computer is down, when the internet is down, and when an analyzer is down? How is it reconciled afterward?
- Do several staff share one OS login? Can each person use a security key or authenticator app?
- Must labels or reports print when nobody is signed in to the LIS app? Are label printers on the network or connected by USB?
- Which quality, billing and identity functions stay in another system, and what happens when it is unavailable?
- Is an analytical repeat already authorized, or does the proposed action order a new test/reflex? How are cancellation, insufficient residual material and expired stability handled concurrently?
- Does the order carry individualized drug classes, diagnosis and signature evidence as received? Which standing-order arrangements exist, who reviews them and when do they expire? Is a point-of-care screen ever followed by laboratory reflex testing?
- Which payer documentation or records requests arrived in the last year, how long did assembly take, and what was missing?
- Does the lab offer medication-consistency comments? Who authored the analyte–medication relationships, and how are unexpected results communicated to clinicians?
- Can a partial report be issued while required components remain unresolved? What does each recipient do with a late final, correction, withdrawal or wrong-person notice?

## Synthetic workflow scenarios

Use the [validation scenarios](../quality/validation-plan.md) to walk through routine and exceptional work: individualized orders, reflex authority, failed QC, partial panels, amendments, duplicate imports, wrong-client access, canceled work, historical export and restored archives. Add power loss during release, replacement-computer restore, update rollback, offline queued orders, destination changes and corrections already viewed by recipients. Record the expected safety outcome and the responsible laboratory role.

## Research deliverables

Produce an observed workflow map, approved scope and data dictionary, host and analyzer compatibility inventory, clinic and EHR inventory, measured workload baseline, counsel's issue list, jurisdiction/accreditation applicability list, revised risks and accepted ADRs. These inputs establish whether the implemented application and remaining proposals fit an approved laboratory pilot. No fixed completion date can be supported until the partner lab and owners are available.
