# Quality and release readiness

This section keeps the current quality contract: required evidence and unresolved
limits. It certifies no clinical validity, compliance or production readiness
([sources of authority](../../AGENTS.md#sources-of-authority)).

| Document | Purpose |
| --- | --- |
| [Validation plan](validation-plan.md) | Stable VAL scenarios, requirement coverage, laboratory acceptance package and stage gates |
| [Open gate items](open-items-g1-g2.md) | Remaining decisions, implementation gaps and qualification work, with stable G1/G2 IDs |
| [Laboratory policy inputs](laboratory-policy-inputs.md) | Unresolved order/result, reporting and recovery policies with stable question IDs |
| [Pending scope decisions](scope-decisions.md) | Source-document custody, urgency/delay communications and patient/proxy disclosure proposals |

The [implementation plan](../planning/implementation-plan.md) describes current
application scope. [Open questions](../planning/open-questions.md) and
[ADRs](../architecture/decisions/README.md) govern decisions; the
[operator guide](../operations/README.md) describes implemented workflows.
Research and source attribution remain in the [research ledgers](../research/README.md).

## Evidence policy

Keep reusable requirements, protocols, current limitations and essential provenance
in tracked documents. Write routine dated reviews, agent handoffs, command logs,
screenshots and benchmark outputs under the ignored `.local/quality-history/`
folder. Canonical documentation must stand alone without Markdown links into
that archive. Move a newly discovered unresolved issue into the applicable current
document before archiving its review.

Local working evidence is not the laboratory's validation record or the project's
release evidence system. Required release evidence must remain retrievable under
the accountable owner's retention and integrity controls; preserve the exact
version, configuration, inputs, commands, outcomes, deviations and approval.
Synthetic fixtures and executable checks needed to reproduce behavior belong
beside the maintained code and test tools.

Use the [quality-gate policy](../../.ai/skills/quality-gate/SKILL.md) to select checks
for each change. Prose cleanup needs documentation checks. It does not require an
application rebuild or establish that application tests passed.

## Verification limits

| Area | Limit that remains |
| --- | --- |
| Clinical use | Synthetic development and automated tests do not replace laboratory validation, approved methods or an authorized production environment |
| Platforms and devices | Native workflows, physical security keys, printing, power loss, replacement-host restore and supported OS/web-engine combinations require their own qualification |
| Performance | Synthetic measurements and mocked browser IPC do not establish all-platform capacity, historical billing/delivery throughput or a laboratory's workload |
| Security and packaging | Signed distribution, native cryptography qualification, release-channel decisions and the open hardening items remain governed by the gate register |
| Product scope | DM-07, RPT-06 and RPT-07 remain proposed; they supply no implemented control or approved external procedure |
