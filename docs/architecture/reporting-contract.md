# Reporting and clinical billing contract

This contract covers REQ-RPT-001–003/005/008/011–013, REQ-BIL-001/002/005, REQ-RES-005, NFR-01 and NFR-07 in the local application. [Report operation](../operations/README.md), [billing operation](../operations/billing-evidence-and-handoff.md), the [domain model](domain-model.md) and [laboratory policy inputs](../quality/laboratory-policy-inputs.md#reporting-and-handoff) describe usage, semantics and decisions still requiring laboratory acceptance.

## Report record and snapshot

`lis-reporting` owns report candidates, immutable released revisions, withdrawal, impact assessments, preservation incidents, destinations and delivery obligations/evidence. Reports are per specimen. The manifest includes the applicable accepted components and reflex triggers; an item linked to multiple accessions blocks release.

| Manifest state | Meaning |
| --- | --- |
| `reported` | Every required analyte has current eligible evidence and approved review. |
| `pending` | Required evidence or review is absent. |
| `held` | Current evidence cannot pass applicable gates. |
| `cancelled` | The component or its item has an accepted cancellation. |
| `trigger_pending` | Reflex authority exists but its trigger remains undecided. |
| `not_activated` | The attributable trigger decision says the condition was not met. |

A final report requires every line to be reported, cancelled or not activated. Partial release works at component level and requires explicit acknowledgement. Each revision is cumulative; a correction to a partial report retains outstanding components.

The snapshot freezes identity, ordering source, specimen and collection context, laboratory profile, medications, exact result revisions, method names/versions/certainty, result forms, limits, validity and condition annotations. The candidate binds its signatory, preparation time, content digest, rendered-data digest, template version, renderer/fonts and artifact digest. Issued reports preserve exact bytes across catalog and template changes.

Printed wording is owned by `lis-reporting/src/display.rs`. Lexical quantities and censored values remain exact. Quantitative cutoff comparison uses **Meets cutoff**, **Does not meet cutoff** or **Not classifiable**; it does not invent positive/negative clinical interpretation. Qualitative source codes retain their distinct meaning. The withdrawal template is fixed in `lis-reporting/src/policy.rs` (`report-notice` v1).

### Report layouts

The report template follows the laboratory's report layout (REQ-RPT-007), owned by `lis-reporting/src/layouts.rs`. The ready-made layouts are **Standard** (`tox-report-standard` v1), **Compact** (`tox-report-compact` v1), **Results first** (`tox-report-results-first` v1) and **Classic** (`tox-report` v3, the default while no layout is approved). The three newer layouts share one data contract with `tox-report` v3 (`lis-report/templates/tox-layouts/contract-v1.typ`, joined with each layout file at build time) and print the same content; only the arrangement differs. Each result row also carries `comparisonCode` (`meets_cutoff`, `does_not_meet_cutoff`, `not_classifiable` or absent), from which those layouts colour the engine's comparison wording; the template never derives it from text.

One laboratory-wide sequence of versions (`rpt_layout_versions`) holds the choice. Configuration staff (`manage_configuration`) draft a layout; quality staff (`manage_quality`) approve it or reject it with a reason. At most one draft waits; an approval supersedes the active version. The draft's author approves only where the separation-of-duties policy allows configuration self-approval and they confirm it (ADR 0015), which a trigger also enforces. Each step is audited (`report.draft_layout`, `report.approve_layout`, `report.reject_layout`).

Preparation reads the layout in force inside its transaction and records its template on the candidate. Rendering uses the candidate's recorded template, so approval never changes a prepared candidate, an issued PDF or its provenance; a revision's provenance names its layout. A layout preview renders the built-in synthetic sample (`lis-report/preview/sample-report-v1.json`) under the pinned preview marking ("PREVIEW – NOT A REPORT"); it reads no laboratory data and stores nothing.

## Printed result expectations

| Evidence | Required presentation |
| --- | --- |
| Quantity/censored value | Exact lexical value and comparator, with the method unit; do not round or convert through floating point. |
| Not performed, invalid, indeterminate or missing disposition | Explicit **Not performed**, **Invalid**, **Indeterminate** or **No result** wording rather than a negative assertion. |
| Method limits | Print only limits actually defined by the frozen method. A defined cutoff of `50` with `ng/mL` prints **Cutoff 50 ng/mL**; an absent limit is **None defined**, not an invented threshold. |
| Validity analyte | Show its measured value/unit and approved validity outcome. A `120.0 mg/dL` observation with no method limits and category `Synthetic SYN-VALID` retains those exact values; the category is not a clinical cutoff. |
| Validity action | `hold`/`recollect` block release; `qualifier` is printed; `comment` prints the category label. |

## Release protocol

Preparation stores the candidate and renders its artifact outside the final writer transaction. Release reads and verifies the artifact, then uses one `BEGIN IMMEDIATE` transaction to recheck:

| Check | Required outcome |
| --- | --- |
| Current authority | The signatory still holds release authority. A stale session principal cannot preserve revoked authority. |
| Fresh confirmation | Authenticate for this action under [ADR 0019](decisions/0019-security-key-authentication.md); password/TOTP and personal-key accounts use their supported confirmation route. Failed confirmation records its refusal without releasing. |
| Candidate identity | Ready state, expected revision/content digest, caller as signatory, correct predecessor, no withdrawal and unexpired candidate. The fixed lifetime is 60 minutes. |
| Frozen content | Rebuild the current snapshot; its digest must equal the candidate's. Re-review after preparation also invalidates frozen review attribution, even if displayed result content is unchanged. |
| Clinical gates | Identity, order, component, specimen/collection, validity, QC/deviation, quarantine and current review all pass. Applicable conditional-acceptance reporting has been explicitly reviewed. |
| Separation of duties | The current laboratory policy decides whether a reviewer may release their own result. Permitted self-release still requires explicit confirmation ([ADR 0015](decisions/0015-separation-of-duties-policy.md)). |
| Recovery | No open recovery reconciliation; the engine applies the operations outbound gate inside the command. |
| Partial release | The actor explicitly acknowledges an incomplete report. |

Only after those checks does the transaction mark the candidate released, insert the immutable revision, append `report.release` and create delivery obligations. A failure leaves no partial release, obligation or successful release event. Domain errors preserve typed gate reasons for the UI.

## Release race expectations

These expected outcomes are the acceptance basis for release-versus-change tests. Prepare candidate C, then race a release with the change below. Audit commit order determines the expected case. The tests exercise both orders and actual concurrent execution.

| Concurrent change | Change commits first | Release commits first |
| --- | --- | --- |
| QC rejection/supersession | Release refuses stale candidate/review (`qc_rejected` or corresponding dependency reason). | Issued bytes remain unchanged; impact review opens for the changed QC dependency. |
| Applicable deviation | Release refuses stale candidate/review (`deviation_opened`). | Impact review opens for that deviation. |
| Identity hold | Release refuses `identity_hold_open`. | Impact review opens for the hold, even if it is later cleared. |
| Accepted cancellation | Release refuses stale candidate/component cancellation. | Impact review opens for the cancelled reported component. |
| Result supersession | Release refuses stale candidate/review (`revision_superseded`). | Impact review opens for the superseded revision. |
| Releaser authority revoked | Release refuses with an audited denial. | Revocation is prospective; it does not rewrite or itself invalidate the issued revision. |
| Held specimen, quality/excursion container hold, clarification hold or validity reassessment | The relevant specimen, container, order or validity gate refuses release. | Impact review opens for the retained change/dependency. |
| Technical re-review | A replaced review cannot be released under the candidate's earlier reviewer attribution. | Earlier released attribution and bytes remain immutable; subsequent consequences follow current review/impact rules. |

While impact remains open, delivery attempts for that revision are refused with `revision_impact_open`. A quality assessment records the exact reason keys it covers; later keys reopen the review. Corrections still need an authorized signatory and every release gate.

Crash expectations: a crash during preparation/rendering leaves no issued revision; a later preparation supersedes an unfinished candidate. A crash during final commit leaves all of revision, obligations and release audit, or none. Missing/corrupt artifact before release is a refusal and preservation incident. Releasing the same candidate twice cannot create two revisions.

## Corrections, withdrawal and preservation

| Action | Contract |
| --- | --- |
| Supplement | Only previously unreported lines become reportable. Link the cumulative revision to its predecessor. |
| Correction | A previously reported line or frozen contextual section changes. Require a reason and preserve the prior version. |
| Unchanged content | Refuse `nothing_changed`; do not manufacture a revision. |
| Withdrawal | Authorized fresh confirmation creates a terminal report withdrawal and minimal notice for every prior destination. Holds do not prevent this safety action. The notice contains no demographics, results or free-text reason. |
| Post-release impact | Compute from retained revisions, assessments and condition history. Changes to the patient identity revision (`identity_changed`) or supplied medication list (`medications_changed`) also open impact review against the frozen report context. Resolving a hold does not erase the fact it affected an issued report. Open impact pauses delivery pending attributable quality assessment. |
| Missing issued artifact | Record a preservation incident and refuse retrieval/export. Never regenerate an issued PDF from current data. |

## Delivery and export

Each obligation identifies its revision and destination. Release routes to active destinations for the order location, or the client where no location destinations exist. Corrections and withdrawals follow every earlier destination and close stale obligations where applicable.

Staff-operated fax, email, mail, courier and pickup channels record channel, observed time, signal source, actor and evidence: handed off by staff attestation, failed, or outcome unknown. Labels identify the channel and evidence; they never imply recipient application processing or a generic **Delivered** state. PDF export is explicitly not delivery.

Destinations require verification under the current authority/separation policy. Replacement creates a new unverified destination; open obligations require explicit redirection. Every attempt rechecks destination/client eligibility and open impact. Export retrieves digest-verified bytes, uses a native save dialog, records the export, and removes a newly written copy if recording fails. Automated hosted delivery, urgent read-back and patient/proxy disclosure retain their separate scope and validation gates.

## Billing derivation and handoff

`lis-billing` owns approved class mappings, deterministic derivations and versioned handoffs. [Billing intake](#billing-intake) covers financial snapshots, evidence, collection corrections and rejection recovery.

| Concern | Required behavior |
| --- | --- |
| Mapping | Use the approved version and required laboratory confirmation under the configured separation policy. Map analytes to project/laboratory class identifiers; ship no CPT content. |
| Service day | Use supplied collection civil date, retaining offset/date precision. Unknown collection evidence is visible; never replace it with intake date. |
| Counts | Keep ordered, performed and reported classes distinct. Derive from performed work and current eligible evidence; classification/positivity does not drive counts. Union classes per applicable service group so repeats add no duplicate service. |
| Exclusions | Cancelled, failed-only, validity and performed-but-unordered work retain their dispositions/flags. A panel name is not a billable class. |
| Clinical independence | Billing deficiencies and reconciliation write billing state; they do not block clinical testing or release. |
| Freshness | Input digests and immutable profile/order versions make derivation idempotent and detect changes. Conflicting same-day responsibility needs explicit reconciliation. |
| Handoff | Preserve the exact exported artifact, derivation/version/source context and per-item acknowledgement, rejection or outstanding status. Adjustments reference earlier items; retries do not create a second service. |

Payer coding, licensed content, claims submission, invoices, remittances and the receiving-system contract remain Q-16/Q-21. A laboratory must validate its service-day/counting rules; software defaults are not payer approval.

### Billing intake

Implemented for REQ-IDN-001, REQ-ORD-005, REQ-SPC-002, REQ-BIL-001/002/005/006 and WF-01/02/12/13. Operator steps, notice handling and payer-record assembly are in the [billing operator guide](../operations/billing-evidence-and-handoff.md).

| Concern | Contract |
| --- | --- |
| Patient financial information | Local, versioned billing profile: address/contact, distinct guarantor and subscriber, bounded coverage records and source/verification information. Clinical identity remains separate. |
| Order billing | Explicitly confirmed snapshot from a selected patient profile revision, selected coverage keys, bill-to responsibility/client and source. Later profile edits do not change an order. Corrections are attributable and participate in billing freshness. |
| Missing information | Billing review tasks with owned resolution evidence; financial deficiencies do not block clinical acceptance, testing or release. |
| Financial evidence | Manual/external eligibility, authorization, provider enrollment and financial-notice evidence, including pre-order estimate evidence that an order later references. The application does not infer eligibility, generate an official notice or declare a balance owed. |
| Collection corrections | Immutable correction history, optimistic revision checks, source and reason. Quality review gates dependent clinical work; technical-review invalidation and report-impact assessment preserve clinical consequences. Billing reconciles both the old and new service-day groups. |
| Export | Versioned handoff with selected financial context and explicit client/provider/location identifiers; original exports remain reproducible. Conflicting same-day arrangements require billing reconciliation without duplicating performed work. |
| Rejection recovery | Append-only later outcomes retain earlier rejections and external references, with concurrency control. Replay does not create a new service. |
| Payer records | Controlled manual assembly is the REQ-BIL-006 boundary; the operator guide defines the steps. |
| Integration boundary | The external billing owner assigns codes, submits claims, invoices and posts remittances. No eligibility network call, cloud service, payer rule content or licensed code set is included. |

**Verification.** Test snapshot preservation after coverage edits, subscriber/guarantor distinctions, permission refusals, malformed/oversized input, stale revisions, audit transactionality, bill-to correction, visible and resolved deficiencies, date corrections across service days, technical-review/report-impact invalidation, complete handoff context, same-day conflict handling and rejected-to-accepted recovery. Keep the existing per-day result-neutral derivation, replay protection and clinical gate tests.

## Work queues and verification

The engine composes owner queries into capability-filtered queues showing state, age and next action. Identity/clinical holds, missing results, stale review, impact, corrections, delivery exceptions, preservation, billing and operational failures remain visible until their owner resolves them. Reconciliation counts distinguish final, partial, unreleased, withdrawn and rejected specimens.

Verify real gates, authorization, migrations, audit, digests, decimal representation and both commit orders. Use synthetic values and independent expected outcomes from this contract and the [validation plan](../quality/validation-plan.md). Implementation reference points: `lis-reporting/src/commands/release.rs`, `lis-reporting/src/policy.rs`, `lis-engine/src/commands/reporting.rs`, `lis-results/src/commands/review.rs`, and current operator guidance.
