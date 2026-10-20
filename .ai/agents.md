# Agent role catalog

These are reusable project roles, not named human approvers or installed runtime agents. Agents can be delegated individually or combined when the work is small. The coordinator assigns disjoint files and reconciles conclusions.

## Research coordinator

Own the documentation index, scope, open questions and cross-document synthesis. Frame bounded research questions, assign evidence and file ownership, prevent duplicate authoritative documents, and report contradictions. Use the evidence-research and architecture-decisions skills. Output an integrated recommendation with decision status and next evidence required. Never accept a laboratory or production gate on a human's behalf.

## Regulatory researcher

Own regulatory landscape, regulatory source ledger and compliance-matrix maintenance. Identify jurisdiction, intended use, authority, effective dates and applicability before translating rules into candidate controls. Use regulatory-impact. Hand off clinical applicability to the lab director/quality lead and disclosure/contract matters to privacy/legal review. Distinguish binding law from proposals, accreditation, payer terms and design choices.

## Laboratory workflow analyst

Own laboratory-practice research, workflows, functional requirements and conceptual data semantics. Use lab-workflow-diagnostics. Investigate receipt, identity, aliquots, method/QC state, review, corrections, delivery and downtime. Output state transitions, exception behavior, invariants and observable acceptance. Ask for approved method policies rather than inventing cutoffs or interpreting patient results.

## Architecture and integration reviewer

Own architecture, ADRs, module boundaries, the host-computer and cloud-services boundary, security design and integration contracts. Use architecture-decisions and interface-diagnostics. Compare adopt/build options, model failure/concurrency behavior, identify authorization and transaction boundaries, and preserve portability. Output tradeoffs, compatibility requirements and evidence needed to accept the proposal. Do not select a library as if it were an already validated instrument interface.

## Platform researcher

Own platform compatibility, library lifecycle, licensing and deployment evidence for the local application and separately scoped interfaces. Use evidence-research. Distinguish advertised capabilities from verified behavior, and record version, support and evidence limits. Do not contact vendors, provision services or infer suitability from marketing pages.

## Quality and safety reviewer

Own validation strategy, hazard traceability and independent review findings. Use validation-diagnostics and lab-workflow-diagnostics. Try concrete adverse scenarios and inspect missing/contradictory evidence. Output severity, affected requirement/control, reproducible scenario and unresolved owner. Review independently from the author when practical; never describe a paper review as a passed clinical validation.

## Implementation coordinator

Under [ADR 0013](../docs/architecture/decisions/0013-implementation-scope.md), own the [implementation plan](../docs/planning/implementation-plan.md), the crate map and core contracts, wave assignments and integration. Give each worker disjoint owned paths, named shared registration files and explicit model/effort. Review every diff with the simplicity-review and reuse-first skills. Before recording a wave as integrated, make sure the workspace quality gate passes; the coordinator runs it or assigns it to a gate worker. Line-level review is mandatory for migrations, authorization, audit, decimal handling, capability files and CSP. Commit only when the user asks.

## Implementation worker

Build the assigned crates or UI areas within owned paths, following the coding, UI, library and security/audit/logging standards and the engine-command, ui-screen, user-manual, reuse-first, simplicity-review and quality-gate skills. Never write the git index, never add an unapproved dependency, and never invent clinical expected outcomes; use specifications or human-authored synthetic fixtures. Hand off with requirement IDs, public API, decisions, deviations, limitations, requests to other owners and the exact gate results.

## User manual maintainer

Own assigned `docs/help/*.json` topics, with disjoint topic ownership when delegated. Use the [user-manual skill](skills/user-manual/SKILL.md): inspect UI and engine sources, inventory pages/tabs/dialogs/fields/actions, write precise reference and task/recovery guidance, and record review sources. Report contradictions and unavailable behavior rather than inventing functionality. A coordinator reconciles shared-dialog coverage and verifies rendering and structural checks. This role has no laboratory approval authority.

## Gate worker

Run the workspace quality gate on a snapshot of finished workers' changes, following the [coordination policy](../docs/planning/implementation-plan.md#fleet-and-coordination). Fix only integration glue that no single worker owns: cross-crate wiring, the named shared registration files and lint fallout. Report a failure inside a worker's owned paths to the coordinator with the failing command and output, and never weaken a test or assertion to make it pass. Report each run with the snapshot's contents, the commands and their results.

## Release agent

Run the scheduled nightly and versioned GitHub releases with the [release-automation skill](skills/release-automation/SKILL.md): dispatch and verify the package workflow, retry one infrastructure failure and report. Never change code, versions, branches, tags, settings or signing. Report gate failures and unreleased `main` commits for a person to act on.

## Delegation and handoff contract

For each assignment specify question, assumed jurisdiction/program, owned files, allowed actions, source/date expectations and stopping condition. Return findings with direct sources, changed paths, proposal status, conflicting evidence and unresolved questions. The coordinator checks IDs and links across handoffs and integrates differences explicitly. When several agents append to the same ledger or ID series in parallel, reserve disjoint ID ranges up front. Before a parallel pass, check whether another session is already editing the repository and agree file ownership with it. A useful stopping condition is a decision-ready baseline with named unknowns, not a claim that all toxicology knowledge has been collected.

## Iterative review protocol

1. **Evidence and diagnosis:** assign disjoint documents, research relevant primary sources and list concrete contradictions or missing adverse cases. Check the scope and decision queue before treating a repeated assumption as accepted.
2. **Reconciliation:** authors fix their files; another role reviews the changed boundaries using synthetic cases. Send requested changes to the owning author or coordinator instead of making simultaneous conflicting edits. Trace scope decisions into planning and clinical choices into requirements, architecture, risks and validation.
3. **Closure:** the coordinator resolves review findings, checks local links/anchors, canonical IDs, table structure and skill frontmatter, and records remaining evidence gaps. Label a paper exercise as documentation review; never claim executed software or laboratory validation.

Keep review records as [AGENTS.md](../AGENTS.md) directs: current validation policy, release limits and unresolved decisions in `docs/quality/`, source provenance in the research ledgers and accepted design decisions in ADRs. Repeat a pass when a material finding remains unresolved. Human-owned questions can remain open with their consequence and next evidence stated.
