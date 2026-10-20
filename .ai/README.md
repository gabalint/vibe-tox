# Project agent guidance

This directory holds the project's tool-neutral agent role cards and skills. These documents do not install agents, start background processes, or guarantee automatic discovery in an agent product. Read the applicable `SKILL.md` explicitly when your tool does not discover `.ai/skills/`.

[Root instructions](../AGENTS.md) govern scope. [Agent roles](agents.md) divide work. Skills describe repeatable methods; roles identify responsibility and handoffs. Qualified humans still own laboratory, regulatory, security and deployment decisions.

| Skill | Use when |
| --- | --- |
| [Evidence research](skills/evidence-research/SKILL.md) | Investigating factual claims, vendor capabilities or changing standards |
| [Regulatory impact](skills/regulatory-impact/SKILL.md) | Evaluating applicability or changes to obligations |
| [Lab workflow diagnostics](skills/lab-workflow-diagnostics/SKILL.md) | Reviewing clinical states, hazards or a workflow requirement |
| [Architecture decisions](skills/architecture-decisions/SKILL.md) | Choosing or revisiting a consequential technical boundary |
| [Interface diagnostics](skills/interface-diagnostics/SKILL.md) | Defining or diagnosing instrument/partner exchange semantics |
| [Validation diagnostics](skills/validation-diagnostics/SKILL.md) | Assessing traceability, test coverage or readiness evidence |

Implementation skills for the scope authorized by [ADR 0013](../docs/architecture/decisions/0013-implementation-scope.md):

| Skill | Use when |
| --- | --- |
| [Reuse first](skills/reuse-first/SKILL.md) | Before writing any code or UI; when duplication or redundancy is reported |
| [Simplicity review](skills/simplicity-review/SKILL.md) | Before handoff and in coordinator review: KISS/YAGNI, spaghetti, complexity budgets |
| [Engine command](skills/engine-command/SKILL.md) | Adding a state-changing command or sensitive query: authorization, gates, audit, outbox, logging, IPC |
| [UI screen](skills/ui-screen/SKILL.md) | Building screens and components from the design system |
| [User manual](skills/user-manual/SKILL.md) | Documenting pages, fields, values, actions, restrictions and recovery alongside UI or engine changes |
| [Quality gate](skills/quality-gate/SKILL.md) | Running and interpreting format, lint, type, test, duplication and unused-code checks |

The research skills require only document access and, for current claims, a suitable research tool; the implementation skills also need the repository's build tools. None carries credential assumptions or authority to contact third parties, upload data, deploy, or clinically authorize results. Future tools can wrap them while preserving this canonical location.
