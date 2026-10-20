# ADR 0002: Modular monolith and durable workers

Status: proposed, 2026-10-12. Decision owner: technical maintainer with laboratory/quality review.

## Context

Identity, method/QC state, result approval, audit and report release have coupled integrity rules. Initial team size and workload are unconfirmed. Operational simplicity and inspectable transactions matter more than independent service deployment at this stage.

The LIS runs as one desktop process on one laboratory computer ([ADR 0009](0009-single-computer-local-lis.md)). The target is 100–250 patients/day; accession, specimen, execution and analyte volumes must be measured separately (NFR-29). LAN/multiple-user topology remains Q-28. The local implementation uses SQLite, which allows one writer at a time with serializable isolation between connections ([ADR 0012](0012-rust-tauri-desktop-application.md), [ENG-99/100](../../research/sources-engineering.md#single-computer-technology-sources)).

## Proposal and alternatives

**One process on the host computer.** The LIS is one desktop application process, with no background service, containing the domain modules, the in-process command interface that the application window calls, and in-process workers ([ADR 0009](0009-single-computer-local-lis.md), [ADR 0012](0012-rust-tauri-desktop-application.md)). The workers handle local outboxes, report rendering, backup snapshots, audit sealing and health checks. A future cloud connector would share the application lifecycle but is not implemented. Narrow adapters handle analyzers and interfaces. Only this process opens the database.

**One database writer.** A single serialized writer connection runs every clinical command in one `BEGIN IMMEDIATE` transaction. That transaction authorizes, checks revisions and gates, writes the change, the audit record and the outbox rows. Workers write through the same writer and never hold it during external I/O. Read connections serve queries and never decide a gate. Prefer explicit module service boundaries and database constraints over unconstrained cross-module table writes.

**Release versus invalidation.** Every conflicting command (release, new hold, superseding QC assessment, identity correction, method or actor authorization change, accepted cancellation) rechecks every gate it depends on inside its own `BEGIN IMMEDIATE` transaction. Because the writer serializes them, the later command always sees the earlier commit. Concurrent-writer designs would require a separate serialization/guard protocol. Tests in both commit orders still apply ([overview](../overview.md#transaction-and-failure-design)).

**Optional hosted services are a separate deployable.** If authorized, they exchange messages with the host only through the published relay protocol and never write clinical state ([ADR 0010](0010-optional-cloud-services.md)). This separation is a product boundary, not a step toward microservices inside the LIS.

Alternatives:

- **Microservices** could isolate scaling and teams, but introduce distributed consistency, observability and recovery work before that need is demonstrated. They also cannot run sensibly on one office computer.
- **Full event sourcing** provides replay possibilities but adds event evolution and projection/rebuild complexity.
- **A generic CRUD application** is simpler initially but insufficient for controlled clinical transitions.
- **Separately deployed worker processes** add inter-process coordination and service management on the host without benefit at this workload.

## Consequences and verification

- **Coordinated releases.** Shared code and database changes ship together in signed per-OS packages, which the lab approves (REQ-OPS-006).
- **Outbox rows are the authority.** Outbox obligations are application-owned rows written in the clinical transaction. An in-memory queue, or a framework callback after commit, does not substitute for them.
- **Interface engines stay outside.** An external interface engine at the integration edge, run by the lab or by a service provider as the EHR interface endpoint ([ADR 0007](0007-integration-edge-and-interface-engines.md)), is not a step toward microservices.
- **Writer capacity.** One writer bounds write throughput. Measure writer-queue waits against NFR-05 and NFR-29 at the small-lab workload, including import bursts, before claiming the workload is qualified.
- **Failure isolation.** A failed worker must not take down listeners. Supervise each in-process worker; outbox leases expire so work resumes after a restart.
- **Evidence.** Prove contested release/QC updates in both commit orders, a crash between commit and dispatch, power loss with `synchronous=FULL`, duplicate messages, and reconciliation. [Architecture transaction design](../overview.md#transaction-and-failure-design) specifies failure behavior.

## Revisit

Reconsider after measured writer contention above the NFR-05 budget, a need for more than one writing host, a true independent team/deployment boundary, or adoption of a platform with a different proven model. If concurrent writers are ever needed, through several writing hosts or a move to PostgreSQL, qualify a guard-row protocol or serializable retry for release versus invalidation. Split services only when measured operational needs justify their added coordination and failure modes.
