# Optional external services: workflow and interoperability evidence

This document retains the evidence behind the proposed optional services in
[ADR 0010](../architecture/decisions/0010-optional-cloud-services.md), which are outside
current implementation scope ([scope](../project-scope.md#product-decision)). Requirement
IDs and acceptance criteria belong in the [requirements](../requirements/functional-requirements.md),
protocols in [integrations](../architecture/integrations.md), and unresolved choices in
[open questions](../planning/open-questions.md). Source checked dates, jurisdiction,
provider claims and retrieval limits are in the
[domain ledger](sources-domain.md#cloud-services-and-interoperability-sources-2026-10-12)
and [regulatory landscape](regulatory-landscape.md). The source research does not
qualify a provider, partner interface or clinical deployment.

## 1. Scope

| Optional service | Boundary to preserve | Open questions and requirements |
| --- | --- | --- |
| Clinic portal | A submitted order is a request until the LIS accepts it. Clinic access is limited to authorized ordering and released reports from the furnishing lab. | Q-05, Q-09, Q-32; REQ-ORD-008 and REQ-RPT-010/014 |
| EHR relay | Retain original payloads and distinguish transport receipt from application acceptance; prove correction and retry behavior per partner. | Q-05; REQ-INT-001/002/008 |
| Report delivery | Each authorized destination gets the correct report revision, with evidence that names the actual channel and confirmation signal. | REQ-RPT-011/012/013; REQ-SVC-009 |
| Managed backup | Add off-host storage or assistance to independently usable local backups; preserve lab-controlled recovery and service exit. | Q-30/Q-31; REQ-SVC-004/005 |
| Patient portal | Identity, proxies, access and release policy need separate approval; existing manual patient access remains part of the LIS. | Q-15/Q-35; REQ-RPT-009 and REQ-SVC-010 |

## 4. Service evidence

### 4.1 Client ordering portal

ONC SAFER order/result-tracking guidance and CLIA test-request content support explicit
order states and sufficient requisition information (DOM-051/052). They do not select
a portal implementation. Toxicology order content follows the same individualized,
source-attributed evidence requirements as local intake: tests/classes, ordering authority,
diagnoses and medications as supplied, collection evidence, and any patient-specific
standing-order or reflex authority (REQ-ORD-005/006/007).

A portal must not preselect clinical order scope or infer diagnoses or medical necessity.
Payer documentation can prompt a billing review without becoming a clinical decision.
The LIS decides whether order authority permits testing. A cancellation, add-on or edited
order remains a request, preserving earlier accepted evidence. Patient identifiers and
cumulative history remain scoped to the clinic's authorization (INV-02/09/13).

#### Order lifecycle, offline behavior and exceptions

For offline operation, a printed requisition must be sufficient for the lab's controlled
manual-entry workflow. Reconcile that entry with the eventual electronic request; do not
silently create a second order. Actual form fields, clinic roles and portal UI remain
future design work.

### 4.2 EHR interface service

#### Partner onboarding and interface qualification

| Evidence | Consequence for future qualification |
| --- | --- |
| HL7 LOI/LRI guide versions differ across the published standards and ONC advisory listings (DOM-009/063; ENG-27–29). | Record the exact guide, message families, optional fields, acknowledgements and deviations for each partner. A generic “HL7 supported” claim is insufficient. |
| FHIR US Core and Order Catalog define resources/profiles with different version and scope boundaries (DOM-064; ENG-28/34). | Resource availability does not prove adoption or a complete laboratory ordering workflow. FHIR remains a separately qualified route. |
| Test orders may omit individualized toxicology evidence or external ordering-authority records. | Qualify a completion route and hold semantics for missing authority, without manufacturing clinical content (REQ-ORD-005/007). |
| Interface software adds a security and maintenance boundary (ENG-19–23/289–296). | Identify an active patch source, license obligations, supported versions, ownership and recovery before admitting a specific engine. No engine is selected here. |

#### Design for a cloud-operated interface endpoint

A hosted endpoint would receive before the local LIS can decide clinical acceptance.
HL7 enhanced acknowledgements distinguish durable receipt from application outcome
(DOM-006). The proposed endpoint therefore acknowledges only what it has durably stored;
the LIS acceptance or rejection follows separately. Prove the partner can handle this
asynchronous outcome. If it cannot, choose an explicit supported fallback.

Keep the original payload and correlation IDs across cloud receipt, LIS receipt and
application outcome. Test duplicates, conflicting reuse of an ID, lost acknowledgements,
offline intervals and corrections to the same report/order identity. Transport success
alone does not establish that a result reached the intended chart or clinician.

### 4.3 Secure report delivery

The retained evidence in section 6 distinguishes available signals from their clinical
meaning. Destination verification, current disclosure authority, retry ownership and
wrong-person withdrawal handling apply regardless of the provider selected. A phone
read-back under the laboratory's critical-result procedure remains distinct from a digital
transport notification (REQ-RPT-004). Actual provider selection and applicable agreements
remain Q-09.

### 4.4 Managed backup for a single-PC lab

Local encrypted backups and recovery are implemented; use the
[backup guide](../operations/backups-and-recovery-kit.md) for current behavior. A managed
service is a proposal that would need separate evidence for off-host custody, retention,
legal holds, monitoring, replacement-host recovery and exit. Existing US contingency-plan
requirements and their applicability are recorded in the
[regulatory landscape](regulatory-landscape.md); storage-provider marketing is not
compliance evidence (DOM-079/082/085/087).

A successful upload or heartbeat does not prove recoverability. An actual restore test
needs usable keys, consistent data, referenced artifacts and integrity/audit checks.
A provider without the decryption key can verify storage-level facts but cannot perform
the lab's decrypted restore; [Q-31](../planning/open-questions.md) holds the verification
options. Record the lab's choice and independent recovery custody. Proposed RPO/RTO targets
need measured exercises; they do not include unmeasured hardware procurement or prove
that an hourly full-copy schedule meets a 15-minute target.

### 4.5 Patient portal (later)

Manual patient access/export requirements remain part of the LIS scope; current software
coverage and unresolved disclosure work are described in the operator and quality guides.
A patient portal needs approved identity proofing, account recovery, personal-representative
and minor access, release timing, accessibility and Part 2 handling before implementation. Use the
[patient-reporting research](patient-reporting.md), current regulatory evidence and Q-15/Q-35.
NIST identity guidance (DOM-088) is a technical benchmark, not proof of a laboratory's
assurance level or a substitute for its legal applicability review. No patient portal or
proofing provider is selected.

## 5. Data flow and minimization

A proposed external service needs a per-service data inventory: copied PHI, purpose,
authorized recipients, retention, subprocessors, support access and exit. A portal that
renders reports or searches orders can access its plaintext data; ciphertext-only backup
has a different technical boundary. Neither description alone settles legal status.
Use the [regulatory cloud analysis](regulatory-landscape.md#15-single-pc-distribution-and-optional-hosted-services)
for applicability and agreements. No PHI belongs in notices, URLs or operational telemetry.

### 5.1 Offline queueing and acceptance

The proposed receipt sequence is portal durable receipt, LIS durable receipt, then LIS
acceptance/clarification/rejection. The clinic must see which event actually occurred.
Retries use a stable identity and content check. An unretrieved request ages into an owned
exception. Local work continues when the internet or service is unavailable; later
reconciliation resolves manual entries and queued requests.

### 5.2 Correction propagation

Released reports remain versioned local records. A correction or withdrawal creates an
obligation for every destination that received an earlier revision. A future portal cache
must mark superseded content as soon as it learns of the change; it cannot learn of a
local change while disconnected, so last-contact and stale-state behavior need explicit
design and qualification (REQ-RPT-013; RISK-43).

HL7/FHIR routes need partner-tested version semantics. Fax, Direct and print cannot
recall a prior physical document; they need a new corrected document or withdrawal notice.
Wrong-patient withdrawals must not disclose the correct patient's information to the
original destination. Missing confirmations remain visible, owned exceptions.

### 5.3 Reconciliation reports per channel

Compare submitted, durably received, accepted, released, transmitted and confirmed items
using their actual identities and report revisions. Keep unknown outcomes and pending
corrections visible with age and owner. Counts alone do not establish completeness.
The [integration architecture](../architecture/integrations.md) owns the proposed channel
state and reconciliation contract; actual cadence and escalation require laboratory policy.

## 6. Delivery channel confirmation

| Channel | Evidence available in the recorded sources | What it does not prove | Sources |
| --- | --- | --- | --- |
| Portal | Proposed authenticated view/download of an identified revision; optional explicit acknowledgement | Reading or understanding; account identity is only as reliable as its authentication and use | Project proposal |
| HL7 v2 | Commit acknowledgement and receiving-system application acknowledgement | Chart placement, clinical review or correction reconciliation without partner testing | DOM-006; ADR 0007 |
| FHIR | HTTP outcome and returned resource/version | Clinical workflow completion or correct partner interpretation | DOM-014 |
| Fax | Provider callback reports transmission success/failure; callbacks may be duplicated or reordered | Correct destination person; providers may not define remote-device confirmation precisely | DOM-069/070/071 |
| Direct | Processed MDN records custody; dispatched MDN records the HISP's defined final delivery | That a clinician viewed it; actual notification and timeout behavior need testing | DOM-067/068 |
| Email | SMTP/provider acceptance by the recipient's mail server; optional disposition notifications | Delivery to a person, reading or a trustworthy guaranteed read receipt | DOM-073/074 |
| SMS | Provider/carrier receipt where available | Reading or reliable end-user confirmation | DOM-075 |
| Printing | Queue/job completion or printer-status signal | Correct physical output, collection or application of a label to the intended specimen | DOM-077/078 |

Record channel, signal source and evidence level separately. Avoid an unqualified
“delivered” label. A timeout or missing confirmation is an unknown outcome to reconcile,
not success. The source ledgers retain provider-specific evidence only as examples of
these signal limits; they do not select or qualify providers.
