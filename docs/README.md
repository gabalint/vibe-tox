# Documentation map

## Start with the current application

| Reader or question | Start here |
| --- | --- |
| What does the application do? | [At a glance](overview.md), then [scope and boundaries](project-scope.md) |
| How do I operate a development installation? | [Laboratory operator guide](operations/README.md) and the [built-in help](help/README.md) |
| How do I build, test or contribute? | [Developer guide](development/README.md), [contribution guidance](../CONTRIBUTING.md) and [implementation status](planning/implementation-plan.md#status) |
| Where are installers published? | [Branches and releases](development/releases.md) |
| What prevents clinical or supported use? | [Quality guidance](quality/README.md), [open gate items](quality/open-items-g1-g2.md) and [validation plan](quality/validation-plan.md) |
| What license and governance apply? | [GPL decision](architecture/decisions/0017-gplv3-license.md), [project governance](governance/open-source-strategy.md) and [development disclaimer](governance/development-disclaimer.md) |

Operator and developer guides describe implemented behavior. Requirements state
intended behavior and acceptance criteria; a requirement row is not evidence that
the feature exists. Accepted decisions and open proposals have separate statuses
in the [ADR index](architecture/decisions/README.md). Research supplies evidence
and applicability limits, not implementation authority.

## Architecture and contracts

| Document | Purpose |
| --- | --- |
| [Architecture overview](architecture/overview.md) | Current host topology, module boundaries, transactions and separately scoped future services |
| [Domain model](architecture/domain-model.md) | Record semantics, identity, analytical provenance and invariants |
| [Order-to-result contract](architecture/order-to-result-contract.md) | Orders, specimens, analytical work, import and review outcomes |
| [Reporting and billing contract](architecture/reporting-contract.md) | Release gates and races, corrections, delivery and performed-work billing |
| [Operations contract](architecture/operations-contract.md) | Backup, recovery, audit anchoring, host health and update records |
| [Backup and export formats](architecture/backup-and-export-formats.md) | Published local data formats used by the application and independent verification tools |
| [UI information architecture](architecture/ui-information-architecture.md) | Navigation, record homes, opening behavior and remaining gaps |
| [Integrations](architecture/integrations.md) | Instrument intake, acknowledgements, reconciliation and proposed partner boundaries |
| [AU400/AU640 protocol](architecture/au-protocol.md) | Supported serial/TCP profile, source qualification and replay behavior |
| [Security and privacy](architecture/security-and-privacy.md) | Trust boundaries, authorization, audit, disclosure and recovery |
| [Deployment](architecture/deployment.md) | Local installation, host operation, backup/recovery and qualification limits |
| [Decision records](architecture/decisions/README.md) | Current accepted choices and unresolved proposals |

## Requirements, planning and quality

| Document | Purpose |
| --- | --- |
| [Workflows and invariants](requirements/workflows.md) | Clinical and operational sequences; conditional external-service workflows |
| [Functional requirements](requirements/functional-requirements.md) | Stable requirement IDs and observable acceptance criteria |
| [Nonfunctional requirements](requirements/nonfunctional-requirements.md) | Workload, performance, recovery and platform targets requiring qualification |
| [Compliance matrix](requirements/compliance-matrix.md) | Source obligations, proposed controls, applicability and human acceptance roles |
| [Implementation plan](planning/implementation-plan.md) | Current scope, crate map, core contracts, milestones and coordination rules |
| [Open questions](planning/open-questions.md) | Project, laboratory and governance decisions still needing owners and evidence |
| [Laboratory policy inputs](quality/laboratory-policy-inputs.md) | Stable order/result, reporting and recovery policy questions for laboratory acceptance |
| [Discovery plan](planning/discovery-plan.md) | Proposed partner-laboratory discovery and selection criteria |
| [Risk register](planning/risk-register.md) | Hazards, controls and validation traces |
| [Pending capability decisions](quality/scope-decisions.md) | Source-document custody, urgency/delay communication and patient/proxy disclosure |
| [Starter catalog contract](planning/starter-data-brief.md) | Draft catalog loading, review/approval and isolated demo data |
| [Result-file contract](planning/result-files-design-brief.md) | Approved import templates, supported file shapes and intake limits |
| [Incremental backup proposal](planning/backup-incremental-design-brief.md) | Proposed extension to current full-copy folder backups |

Stage gates from development to clinical use are in the
[validation plan](quality/validation-plan.md#stage-gates).

## Engineering and evidence

The [coding](governance/coding-standards.md), [UI](governance/ui-standards.md),
[page layout](governance/page-layout-standard.md),
[library](governance/library-policy.md),
[security/audit/logging](governance/security-audit-logging.md) and
[engineering](governance/engineering-standards.md) standards govern changes.
Maintain user-visible documentation through the [contextual-help
contract](development/contextual-help.md).

The [research index](research/README.md) links retained clinical, regulatory and
technical evidence. Source ledgers preserve [regulatory](research/sources-regulatory.md),
[domain](research/sources-domain.md) and [engineering](research/sources-engineering.md)
attribution, dates and limits.

[AGENTS.md](../AGENTS.md) governs agent work; [`.ai/`](../.ai/README.md) holds the
project roles and skills. Keep stable requirement, risk, workflow, source and
decision IDs when revising documents. The [quality evidence
policy](quality/README.md#evidence-policy) defines where working records belong.
