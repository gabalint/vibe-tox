---
name: lab-workflow-diagnostics
description: Diagnose gaps in toxicology laboratory workflows, state transitions and result-safety requirements using synthetic scenarios.
---

# Lab workflow diagnostics

Read [workflows](../../../docs/requirements/workflows.md), [functional requirements](../../../docs/requirements/functional-requirements.md) and [domain model](../../../docs/architecture/domain-model.md). Select the affected program and follow one synthetic accession through relevant states.

Check actor qualification, patient/specimen/aliquot identity, versioned configuration, QC state, allowed transitions, retained evidence and correction behavior. Separate presumptive/definitive from qualitative/quantitative; distinguish missing, not performed, invalid, below a limit and a true numerical value.

Try the adverse cases material to the change: mismatched barcode, duplicate order, late result, unit/decimal mismatch, failed QC, repeat/dilution, cancellation, concurrent approval, wrong-recipient delivery or outage. For outpatient drug monitoring, also try:

- a practice-wide profile used as a standing order
- a missing or prepopulated diagnosis
- a point-of-care screen followed by a lab reflex
- an untested prescribed drug in a medication-consistency check
- a reprocessed upstream batch re-export
- a payer records request

State what must block, quarantine, create a new version or require attributable review. Do not invent assay cutoffs or substitute an administrative override for laboratory policy.

Return findings with a concrete scenario, affected REQ/WF IDs, expected behavior, evidence gap and responsible role. Record material hazards in [the risk register](../../../docs/planning/risk-register.md). Use synthetic examples only. A diagnostic review is not completed method or clinical validation.
