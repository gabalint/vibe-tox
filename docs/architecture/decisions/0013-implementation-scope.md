# ADR 0013: Implementation scope and engineering baseline

Status: **accepted in part**.

- **Accepted by the project owner, 2026-10-12:** implement the application on synthetic data. Development standards for coding, UI and libraries, and skills that prevent duplicate, redundant, tangled and complex code, follow KISS and YAGNI, and treat security, audit and logging as first-class.
- **Proposed:** milestone boundaries and component choices under [ADR 0012](0012-rust-tauri-desktop-application.md), subject to technical-maintainer review.

Plan: [implementation plan](../../planning/implementation-plan.md).

## Context

The authorized application scope is a single-computer application with explicit requirements, contracts and verification. M0–M5 components are integrated; current scope and remaining gaps are maintained in the [implementation plan](../../planning/implementation-plan.md#status). Implementation does not establish clinical validation or production readiness.

## Decision

1. **Scope.** Build the [implementation plan's scope](../../planning/implementation-plan.md#scope) on synthetic data, in milestones M0–M5. The cloud services, LAN workstations, the HL7/FHIR routes beyond M3 needs, the patient portal and code signing require a new explicit user instruction. Updating a plan alone does not authorize them.
2. **Foundation verification.** ADR 0012 acceptance items are tested in the application foundations:
   - decimal round-trip;
   - outbox crash recovery;
   - release in both commit orders;
   - ASTM at the sign-in screen;
   - a deterministic Typst PDF;
   - ZPL bytes to a printer;
   - a 10,000-row grid.

   Items needing real hosts per OS (power cut, signing/notarization, WebKitGTK performance) remain open qualification gates.
3. **Engineering baseline.** These standards are in force:
   - [coding standards](../../governance/coding-standards.md);
   - [UI standards](../../governance/ui-standards.md);
   - [library policy](../../governance/library-policy.md);
   - [security, audit and logging](../../governance/security-audit-logging.md);
   - [engineering standards](../../governance/engineering-standards.md) for process and release;
   - skills in [`.ai/skills/`](../../../.ai/README.md).
4. **Component baseline.** The [library policy](../../governance/library-policy.md) and [ADR 0012](0012-rust-tauri-desktop-application.md) identify the local components and their remaining qualification boundaries. Current code uses module-per-crate ownership, table prefixes and per-module migrations; generated TypeScript bindings connect the Rust engine and UI. Do not maintain a competing dependency inventory in this scope decision.
5. **Process.** Waves of parallel workers with disjoint owned paths. The coordinator reviews and integrates every wave and records status in the plan. Workers never write the git index or history. External fleets require explicit authorization under the current [coordination policy](../../planning/implementation-plan.md#fleet-and-coordination).

## Consequences

- Code stays within the plan's scope and follows the domain invariants and synthetic-data rule in AGENTS.md.
- Unresolved questions and partner choices stay open:
  - menu, analyzers and partner (Q-02 to Q-05);
  - ownership, contribution and remaining licensing governance (Q-11);
  - signing route (Q-19);
  - off-host audit anchoring without a hosted service (Q-30).

  Code keeps them configurable or explicitly deferred, never silently decided.
- Generated volume is not evidence. Each milestone exits only on the evidence listed in the plan.

## Revisit

- The technical maintainer or project owner changes the component choices.
- A milestone shows a contract (command context, migrations per module, outbox, IPC) does not hold.
- The project owner re-prioritizes the optional cloud services or a partner's requirements.
