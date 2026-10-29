# Billing evidence and handoff

Procedure draft, 2026-10-14. The laboratory and its billing owner must approve the operating policy, responsibilities, applicable payer rules and disclosure process before real-data use. This implements the controlled manual records boundary of REQ-BIL-006; it does not submit claims or determine coverage.

## Where information belongs

| Level | Operator action | Preserved evidence |
| --- | --- | --- |
| Patient | Record supplied billing address, guarantor, coverage and subscriber details. Record source and verification dates; retain manual estimate/notice evidence that arrives before an order. | Versioned financial profile. Subscriber and guarantor are distinct people/roles. |
| Order | Explicitly select responsibility, bill-to client when applicable, profile version and coverage priority. Confirm source and reason; retain only the manual financial evidence applicable to this request. | Immutable order-specific snapshot. Later patient changes do not silently update it. |
| Accession | Confirm actual collection evidence. Correct a transcription error using the source document and reason; obtain quality assessment of the changed clinical timing. | Original collection facts, correction revisions and separate quality review. Technical-review and released-report consequences remain visible. |
| Service day | Review derived ordered/performed work, final-report status and financial deficiencies across the patient's specimens. Resolve differing financial arrangements with the billing owner without duplicating service counts. | Derivation input digest, approval, attributable reconciliation and prior handoff links. |
| Handoff | Export the approved versioned JSON to the authorized billing workflow. Record the recipient's actual acknowledgement or rejection and reference. | Exact stored artifact, export events, original and later outcomes. A transport acknowledgement alone does not establish claim processing. |

Financial deficiencies are billing work, separate from clinical identity, QC and release holds. Missing insurance must not be silently replaced with self-pay. Clinical order scope, diagnoses and medical necessity remain what the ordering source supplied.

## Missing information and corrections

Open a billing follow-up with the affected category and supplied note. Assign a person/team, retain the requested source, and resolve only with an attributable reason and evidence reference. Reopening retains prior resolution actions. Review the oldest open work and continue through all queue pages; a first page is not the complete workload.

A changed patient profile is reusable information for future selection. To correct an existing order, open its billing context and explicitly select the revised profile/coverage and responsibility. This creates a new order billing revision, invalidates prior billing approval when relevant, and preserves earlier evidence.

A collection correction requires accession authority and source evidence. Quality staff assess timing effects, including specimen suitability under the laboratory's policy. Resolving that review does not restore an old technical review or erase the need to assess a released report. Complete those clinical reviews and any necessary report amendment separately.

After a service-date correction, reconcile **both** dates. If all previously handed-off work leaves the old date, its adjustment explicitly voids those prior services. The new date's work remains subject to the normal clinical and financial checks. Keep the original export and send linked adjustments through the billing owner's agreed process.

## Manual coverage and notice evidence

Eligibility, prior authorization, provider enrollment and financial notices are manual/external evidence records. Record the actual source, status, service scope, relevant coverage/provider, reference, applicable dates/units and observed time. Unknown or pending evidence is not approval; recorded evidence is not proof that a payer will pay.

Where a notice applies, retain its actual form version, service-specific explanation, estimate/currency, patient choice, provision/signature times and signer, plus the authoritative source document reference. The LIS does not create an official form, infer a signature, compute liability or determine whether a notice is legally sufficient. A pre-order estimate request/evidence can be retained at patient level and explicitly referenced when an order exists.

For US Original Medicare, ABN applicability and completion follow the current CMS instructions; other plans/programs have different rules. Good-faith-estimate obligations can arise before an order and must be handled in the scheduling/request workflow. Applicability depends on the patient, service, payer and jurisdiction. Sources and their checked dates are recorded in the [domain](../research/sources-domain.md#billing-intake-and-financial-data-ownership) and [regulatory](../research/sources-regulatory.md#billing-intake-review) source ledgers.

## Rejected handoffs

Record the rejection and external reference. Determine whether the billing owner needs an administrative correction/retransmission of the same service, or whether changed clinical/financial facts require a fresh derivation and linked adjustment. Record subsequent rejection or acknowledgement against the original item with a reason; the earlier response remains in history. Do not create a second original service simply to clear rejection work. Acknowledgement is terminal for that item; later substantive changes use adjustments.

## Payer record requests and appeals

This is a controlled manual assembly process. The application does not promise an automatically complete payer packet.

| Step | Required operator check |
| --- | --- |
| Register request | Record requesting party, verified destination, patient/service scope, request reference, due date and assigned owner in the laboratory's approved request tracker. Do not put PHI in support tickets or diagnostic logs. |
| Verify authority | Follow the laboratory's approved disclosure policy, applicable law and payer contract. Confirm the request concerns this lab's patient/services. |
| Identify exact versions | Match subject identifiers, order and accession numbers, actual collection dates, derivation and handoff item IDs, original/adjustment references and external claim references. |
| Gather supplied evidence | Obtain requisition/ordering authorization, supplied diagnoses and relevant medical-necessity documentation, coverage/subscriber/guarantor selection, authorization/notice evidence and original source documents. A reference in the LIS does not itself mean the external document is present or sufficient. |
| Gather laboratory evidence | Use the actual performed-work derivation, collection/correction history, released report revision(s) and amendments, review/impact records relevant to the request, and the exact stored billing handoff(s). |
| Reconcile | Check dates, identifiers, units/service scope, order provenance, report versions and every prior adjustment/outcome. Explain gaps to the responsible owner; never invent a diagnosis, signature or missing source. |
| Review and disclose | Have the authorized reviewer approve the packet, verify recipient/address and permitted scope, then transmit through the approved channel. Record exact packet/version, recipient, method, date, operator and delivery evidence in the approved disclosure tracker. |
| Close or appeal | Record response, unresolved items, follow-up deadline, appeal reference and disposition. Preserve the packet and source versions under the laboratory's approved retention schedule. |

External billing owns code assignment, claim creation/submission, invoices, remittances and payment posting. Payer/MAC/state-specific requirements, financial policy and retention schedules remain laboratory decisions under [Q-16/Q-21](../planning/open-questions.md).

## Reach the exact billing work item

Billing work is grouped by patient and collection-derived date of service, including an explicit unknown date. Queue links select that exact pair; a second date for the same patient is a different item. **Load more** continues beyond the first page with timestamp, patient and date ties preserved. **Refresh** rechecks current work and permissions. Recheck affected evidence before deriving or approving after a conflict; never recreate an order or service to reach a hidden item.

A handed-off item leaves Billing work once the partner's acknowledgement is recorded. **Billing › Billing handoffs** lists every handoff, newest first, and finds one by its number, the start of an accession number or the start of a `YYYY-MM-DD` date of service. Open it to export the stored file again or review each item's outcome history.

The external assembly steps above describe requirements for a laboratory-owned process, not an approved SOP supplied by this project. Managed source custody (DM-07) and patient/proxy request handling (RPT-07) remain separate scope decisions in the [scope decisions](../quality/scope-decisions.md). They are not closed by this guide.
