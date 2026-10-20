# ADR 0005: Versioned provenance and controlled report release

Status: proposed, 2026-10-12. Owners: laboratory director, quality and technical maintainers.

## Context and proposal

Methods, cutoffs, mappings and report templates change over time. Result corrections must remain understandable after such changes. Retain source observations and frozen clinical configuration references; use immutable released report versions and append new corrections with reasons and recipient reconciliation.

Prepare an approved candidate snapshot and retained report artifact before final release. Atomically recheck current holds/authorization/revision, record release and enqueue delivery. Separate release from delivery success. Use durable inbox/outbox patterns and idempotent processing; external interfaces remain at least once with reconciliation. See [the exact publication sequence](../overview.md).

Release and gate-changing commands share one conflict protocol across QC, identity, method/actor authorization and order state. Invalidation committed before release blocks the candidate; invalidation committed afterward creates impact work for the released revision. Review is tied to selected result/dependency revisions. Expected-component dispositions determine completeness independently from publication and correction status. Candidate artifacts are retained by digest in the local artifact store; files must be synchronized before database rows reference them because file writes and database release do not commit atomically.

Imported results also record the upstream analytical review that produced them. This covers the review system/version, processing revision and whether certification was human or rule-based ([domain model](../domain-model.md#upstream-analytical-review-and-re-export)). LIS technical review remains a separate decision. Release manifests and audit events join the hash chain anchored to write-once storage proposed in [ADR 0008](0008-tamper-evident-audit-and-immutable-recovery.md).

Sequence revisions per recipient, explicitly reconcile unknown remote outcomes, and recheck current disclosure authority on dispatch/retrieval. Historical recipient obligations do not grant continuing access, particularly after wrong-person correction. Recovery must identify externally issued revisions using independent manifests/receipts and fence old workers before resumed delivery.

## Alternatives and consequences

Overwriting a report or rendering it from today's configuration destroys historical meaning. Writing only a PDF loses structured provenance. Full event sourcing is unnecessary for this initial model. Retaining originals and revisions costs storage and requires class-specific retention/legal-hold design. Report corrections must flow to each destination and may need manual escalation when a partner cannot acknowledge correction semantics.

## Acceptance and revisit

Demonstrate historical retrieval after method/template changes, decimal-boundary precision, source-file replay, release/gate races in both commit orders, artifact retention failure, partial-to-corrected recipient ordering, wrong-person correction without redisclosure, revocation and restore before acknowledged release. Laboratory-approved policy defines technical review, final authorization and permitted exceptions. The proposal implements findings in [workflows](../../requirements/workflows.md) and [compliance](../../requirements/compliance-matrix.md); those owners must confirm applicability before acceptance.
