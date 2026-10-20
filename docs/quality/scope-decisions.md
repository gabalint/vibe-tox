# Pending scope decisions

Status: **proposed; awaiting project-owner and laboratory decisions**. These
three gaps remain open. None of the recommendations below is an implemented
control, an approved laboratory procedure or evidence of closure.

The governing requirements are REQ-ORD-005/006, REQ-OPS-003, REQ-RPT-004 and
REQ-RPT-009 in the [functional requirements](../requirements/functional-requirements.md).
The [patient-reporting evidence](../research/patient-reporting.md) and
[regulatory source ledger](../research/sources-regulatory.md#patient-reporting-sources)
retain clinical and regulatory provenance. Laboratory policy and applicable
jurisdiction determine the operational details; no state or accreditation
program is selected.

## DM-07: source-document custody

Owner: project owner and laboratory records owner; records custody is also part of
Q-14. A document-reference string
does not preserve the source document. Existing external rule-approval references
do not resolve patient-record or source-document custody.

| Decision element | Proposed scope or acceptance |
| --- | --- |
| Recommended scope | Explicitly import a source file into the existing local artifact store, preserve its original bytes and digest, link a version to patient/order evidence, and retrieve it through an audited view/export. Include retained bytes and links in backup, restore and exit export. Host disk encryption protects the local store. |
| Required metadata | Preserve author, time, source, content type, original filename as sensitive metadata, and replacement/supersession history. |
| Boundaries | Synthetic files during development. No OCR, interpretation, scanner integration, remote storage or implicit deletion. The project owner selects allowable formats and a size budget before implementation. |
| Acceptance | Test duplicate imports, rejected types and size limits, interrupted persistence, exact-byte restore, denied access and complete export of retained files and links. |
| Alternative | Declare an external-record dependency with the source system/reference and responsible custodian, availability checks and visible failures. The lab must approve and qualify retrieval, retention and continuity. Do not claim that LIS backup or exit export contains externally held source documents. |

## RPT-06: urgent-result and delayed-testing communications

Owner: project owner and laboratory clinical/quality owner; critical-result policy
is also part of Q-08. Delivery-attempt
records alone do not demonstrate clinical urgency assessment, delayed-testing
follow-up or accountable escalation.

| Decision element | Proposed scope or acceptance |
| --- | --- |
| Recommended scope | A local communication register linked to the accession, report or result. Record who assessed urgency/delay under a referenced approved lab policy, the intended recipient, contact attempts with time and method, acknowledgement/read-back where applicable, unresolved status and escalation. |
| Boundaries | No invented thresholds, automated medical interpretation or external messaging. Preserve the released report. Human owners supply the clinical criteria and contact/escalation policy. |
| Acceptance | Distinguish an attempt from successful communication, allow multiple attempts, and require attributed closure with a reason. Test reopen/retry, current authority, denied actions and reachability of overdue or unresolved work. |
| Alternative | Defer implementation only with a laboratory-owned controlled external process and a tested evidence/reconciliation interface. Agent-written documentation cannot approve that process or close the gap. |

## RPT-07: patient and proxy disclosure

Owner: project owner and laboratory privacy/records owner; patient access is also
part of Q-15 and the later patient portal is Q-35. A final PDF alone does
not establish a complete patient record set; a full-host export is not a
patient-specific disclosure package.

| Decision element | Proposed scope or acceptance |
| --- | --- |
| Recommended scope | A local patient/proxy request register and reviewed disclosure package. Record request date, requester/authority evidence reference, scope, responsible owner, applicable due date, and extensions with reasons. Assemble explicitly selected records and exact issued artifacts; log review and completed disclosure. |
| Boundaries | No portal or identity-proofing service. No automatic conclusion about proxy authority or universal deadline. Policy and date calculations identify the applicable federal/state context. Human review remains required. |
| Acceptance | Test wrong-subject exclusion, proxy denial, partial fulfillment, retained requests, record-set completeness and exact artifact provenance. |
| Alternative | Defer with a laboratory-owned external request/disclosure process and tested complete record retrieval. The manual route must remain available without requiring a future patient portal. |

Approval of one proposal does not authorize the others, cloud services, real
patient data, publication or third-party communications. Retention periods,
record-set boundaries and clinical communication criteria need accountable
laboratory policy and applicable-jurisdiction review.
