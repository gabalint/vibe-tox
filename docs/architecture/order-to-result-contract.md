# Order-to-result contract

This document specifies the local order, specimen, analytical and result boundaries for REQ-ORD-001–007, REQ-SPC-001–005/008, REQ-ANA-001/003–007 and REQ-RES-001–005. Laboratory policies and instrument qualification remain open in the [laboratory policy inputs](../quality/laboratory-policy-inputs.md#order-to-result) and [release gates](../quality/open-items-g1-g2.md).

Use the [daily-operation guide](../operations/daily-operation.md) for operation, the [domain model](domain-model.md) for record semantics, and the [implementation plan](../planning/implementation-plan.md#core-contracts) for shared command/query contracts. Current migrations and generated TypeScript bindings define exact database and IPC shapes; this contract defines the safety outcomes those implementations must preserve.

## Orders

`lis-orders` owns supplied order scope and evidence, accepted versions, clarification holds, standing orders, follow-on requests and cancellations. Acceptance freezes the approved orderable and component versions. Later catalog changes do not silently rewrite accepted work. [ADR 0018](decisions/0018-remediation-authority-and-method-continuity.md) governs configuration authority, method continuity and patient-specific standing-order applicability.

| Concern | Required behavior |
| --- | --- |
| Supplied evidence | Preserve requestor, source, diagnoses, medication statements and client point-of-care results as received. Do not infer a diagnosis, order scope, reflex authority or medical necessity. Client point-of-care observations remain context, not laboratory result lines. |
| Holds | Authorization/clinical clarification is distinct from financial documentation deficiencies. Billing review never blocks clinical acceptance, testing or release. |
| Standing orders | Scope, subject, practitioner and available uses are checked at acceptance. Collection evidence controls the validity period at testing/release; an accepted use is reserved and is not revoked merely because remaining uses become exhausted. |
| Follow-on work | Add-on, reflex, technical repeat and recollection are distinct operations with their own authority and provenance. Reflex activation is attributable and idempotent. |
| Cancellation | Retain observations received after cancellation; cancelled components cannot pass review or release. A cancellation never erases a released report. |

## Specimens

`lis-specimens` owns accession, collection, specimen, container, aliquot, receipt decision, hold, movement and label-job records. These identities remain distinct. Collection precision, original offset and unresolved time-zone ambiguity remain explicit. Source-backed corrections preserve prior values and require clinical/billing consequence review.

| Concern | Required behavior |
| --- | --- |
| Material identity | Patient run positions use the printed container label. Reprints retain that identifier and record actor and reason. Aliquots preserve parent lineage. |
| Receipt | Accepted, conditionally accepted, held and rejected decisions remain attributable. Conditional acceptance requires policy context and explicit report-condition review where applicable. |
| Material use | Placement rechecks suitability, active holds and the accepted order. Volume arithmetic uses exact decimals and equal unit text; unknown volume does not imply depletion. |
| Holds and disposal | Quality and excursion holds affect clinical eligibility. Retention/legal holds protect custody. Disposal refuses open holds and retains evidence. |
| Recovery | Accession and container number floors only increase; restore reconciliation protects labels issued after the restored snapshot. |

## Analytical work

`lis-analytical` owns runs, positions, executions, preparation batches, lots, QC assessments, deviations and approved source registrations. Method certainty, output scale and sequence role are separate properties. Frozen method versions remain attached to accepted work under ADR 0018.

Patient, control, calibrator and blank positions are established before acquisition. The run position controls routing; external payload text cannot turn QC into patient evidence. QC assessments are append-only and identify their evidence. Rejection, supersession or a new applicable deviation invalidates dependent review. Administrator configuration authority does not waive QC.

Rerun, re-extraction and dilution are new executions with lineage and reasons; reprocessing is a new processing revision. The LIS does not multiply a received value by a dilution factor. A source must declare final concentrations and satisfy applicable dilution checks.

## Result evidence and review

`lis-results` preserves received measurements separately from selected result revisions and technical-review decisions. Values retain exact decimal text, comparator, unit, precision, method limits and original source text. Missing evidence is not a negative result. Cutoff comparison follows the frozen method rule; unsupported compare-after-rounding cases remain not classifiable.

A review binds exact revisions and dependency IDs. Approval rechecks current revision, open quarantine, order/specimen eligibility, QC, preparation, deviations and required validity assessment. Current review is selected by writer-transaction sequence, not host timestamps. A repeated or backward clock cannot restore an older decision.

Review becomes stale when its revision or required assessment is superseded, QC is no longer acceptable, or an applicable deviation opens. Staleness is computed from durable evidence. Release consumes that status and rechecks its own gates in the [reporting contract](reporting-contract.md#release-protocol).

## Receipt and import

A durable receipt precedes acknowledgement and parsing (REQ-INT-009). The engine reads the content-addressed artifact with digest verification, parses outside the writer transaction, and imports typed rows in an authorized, audited transaction. Import attempts are idempotent by receipt and attempt number; infrastructure retry does not fabricate a second observation.

| Routing boundary | Required behavior |
| --- | --- |
| Registered source | ASTM sender/version and test-code mappings must match the approved source. File receipts use an approved layout. Unsupported or mismatched sources remain visible for reconciliation. |
| Patient labels | Match the selected sample-ID field against open runs of the sending instrument. No match is `unknown_sample`; several matches are `ambiguous_sample`. Never match by row order. |
| QC | Route control, calibrator and blank positions to QC observations. Measurement storage refuses nonpatient positions; result revisions cite patient measurements only. |
| Units | Compare exact configured unit text. Do not infer conversions. |
| Outside order scope | ASTM rows enter the Not ordered queue. File sources can retain usable measurements as **measured, not ordered**; only an accepted add-on and an attributable reviewer action can use them, followed by ordinary review and release gates. |
| File-specific semantics | Value words, declared units, role/dilution checks, row limits and watched-folder reconciliation follow the [result-file contract](../planning/result-files-design-brief.md). |

## Re-export outcomes

These are independent acceptance expectations for duplicate, correction and partial-import tests.

| Input | Expected outcome |
| --- | --- |
| Byte-identical receipt | Mark the duplicate receipt; do not enqueue a second import. |
| Redelivered import work item | The receipt/attempt key makes processing a no-op. |
| Identical rows in new bytes | Count duplicate rows without adding evidence. |
| Changed row with an approved ASTM correction indicator | Add a processing revision and new measurement with supersession. If that measurement supplied the selected result, advance its result revision and invalidate the old review. |
| Changed row without that indicator | Quarantine `conflicting_duplicate`; preserve current evidence until a reviewer accepts reprocessing with a reason. |
| Partial re-export | Process included rows only. Omission never deletes, cancels or means negative. |
| Source explicitly says not performed | Retain a `not_performed` observation as source evidence. |
| Unregistered sender/version | Quarantine the attempt; retry the retained receipt after configuration approval. |
| Changed QC row | Add superseding QC evidence and make the prior dependent QC assessment stale. |
| Evidence from another execution | Keep it; selecting it for a current result is attributable and requires a reason. |

## Verification and boundaries

The test journey covers order → accession → label → durable import → QC → review, including duplicate, partial and reprocessed exports. Tests must exercise the real migrations, authorization, audit transaction, decimal parser, dependencies and gate functions. UI IPC and transport can use synthetic harnesses; expected clinical outcomes come from this contract and the [validation plan](../quality/validation-plan.md), not from the code under test.

Unimplemented capabilities retain their requirement and release-gate status: partner-defined automatic reflex/interpretive rules, per-method competency, retention/disposition schedules, record merge, and separately scoped portals or external exchange. The [laboratory policy inputs](../quality/laboratory-policy-inputs.md#order-to-result) retain unresolved laboratory choices rather than treating synthetic defaults as approval.

Implementation reference points: `lis-results/src/commands/review.rs`, `lis-results/src/queries/review.rs`, `lis-results/migrations/0007_review_commit_order.sql`, the public domain module contracts, and the current operator guides. API and schema changes require owner review and relevant contract tests.
