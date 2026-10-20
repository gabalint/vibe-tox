---
name: architecture-decisions
description: Create or review a consequential LIS architecture decision with alternatives, domain invariants, operational consequences and acceptance evidence.
---

# Architecture decisions

Read the [overview](../../../docs/architecture/overview.md), [ADR index](../../../docs/architecture/decisions/README.md) and relevant requirements. Distinguish an accepted constraint from a proposal. The single-computer product shape ([ADR 0009](../../../docs/architecture/decisions/0009-single-computer-local-lis.md)) is a user decision. Check that a proposal keeps the lab computer as the system of record and keeps safety and exit functions in the local application ([ADR 0010](../../../docs/architecture/decisions/0010-optional-cloud-services.md)). Follow the accepted application scope and the [reuse-first skill](../reuse-first/SKILL.md).

Compare credible alternatives against clinical integrity, authorization, transaction/retry behavior, portability, maintainer capability, operating burden, licensing and lifecycle cost. Identify what can fail halfway through the proposed operation and what durable state remains. Quantify only supported estimates and label planning targets.

Write an ADR when the decision creates a durable boundary or difficult-to-reverse dependency. Include context, status, owner, proposal, alternatives, consequences, decisive source links, acceptance evidence and revisit triggers. Keep routine reversible document edits lightweight.

Update the overview and affected requirements when a decision changes. Keep tracked decisions focused on current scope and proposals. Archive obsolete project-choice records locally after preserving any enduring constraints and source provenance in the current documents; repair their incoming links and do not reuse decision IDs. Do not mark a proposal accepted without actual authority or implement beyond the authorized scope. Return a decision-ready recommendation and unresolved tradeoffs, not an invented implementation result.
