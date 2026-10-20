# ADR 0009: The LIS is a desktop application on one laboratory computer

Status: **accepted in part**.

- **Accepted (product shape and application form):** by the project owner on 2026-10-12: a standalone desktop application in one process, with no background service, on Windows, Linux and macOS.
- **Operating-design acceptance remains open:** the offline-first rule, the single-writer rule, the update model and the host baseline await technical-maintainer/project-owner acceptance. The local implementation already contains a single writer and offline workflows; implementation does not close the qualification gates.

Related: [ADR 0010](0010-optional-cloud-services.md) defines optional cloud-service boundaries; [ADR 0012](0012-rust-tauri-desktop-application.md) defines the desktop stack.

## Context

Running the LIS on one laboratory computer gives the laboratory portable records and local control ([product scope](../../project-scope.md#product-decision)). The standalone cross-platform application is the system of record; optional external services are separately scoped proposals.

## Decisions (accepted by the project owner, 2026-10-12)

1. **Product shape.** The LIS is free, open-source software that a laboratory installs and runs on one computer it controls. It is not a hosted service. Optional future external services are separately scoped proposals ([ADR 0010](0010-optional-cloud-services.md)).
2. **Standalone desktop application.** The LIS is a desktop application with its own window, not a web application in a browser.
3. **One process, no background service.** The analyzer listeners, database, outbox and backup scheduler run inside the application. Analyzers work only while the application is open. The application may be open with nobody signed in; people sign in to act, so each action is attributed to a person.
4. **Cross-platform.** The application runs on Windows, Linux and macOS.

## Proposal (operating rules)

1. **Offline-first.** Every local laboratory workflow works without internet: accessioning, analyzer import, review, release, printing, local delivery, backup to local targets and export. Cloud services degrade to visible queues.
2. **Local system of record.** Clinical state exists only on this computer and its backups. Cloud copies are projections or requests with defined retention ([ADR 0010](0010-optional-cloud-services.md)).
3. **Single writer.** One writer thread runs every clinical command in one write transaction. That transaction rechecks all release and invalidation gates, then writes the change, the audit and the outbox rows ([ADR 0002](0002-modular-monolith.md)).
4. **Open-application rules.**
   - Prevent system sleep while listeners are active.
   - Warn before closing with active listeners or pending outbound work.
   - Allow one running instance per database.
   - On every start, reconcile interrupted imports and expected results.
   - Starting at OS sign-in and minimizing to the tray or menu bar are lab options.
   - The OS screen may be locked: the app keeps running. An OS sign-out, reboot or update stops listening ([stack research](../../research/desktop-application-stack.md#one-process-what-it-means)).
5. **LAN workstations later.** Up to 3 other lab workstations could connect to the host instance over authenticated TLS while the host app is open (Q-28). The database is never shared over a network file system, because SQLite's WAL mode does not work on one ([ENG-97/99](../../research/sources-engineering.md#single-computer-technology-sources)).
6. **Lab-approved updates.** The application checks a signed update manifest. It never installs on its own. A lab administrator approves each version after reviewing release notes and the validation package. CMS surveyor guidance expects the lab to accept new or revised LIS programs before use ([regulatory landscape §15.4](../../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system)). The application takes a verified snapshot first.
7. **Host baseline.** A supported desktop OS on x64 or Apple silicon, with full-disk encryption, an SSD and a UPS. Exact host/OS qualification is Q-29. The [deployment baseline](../deployment.md#host-baseline) owns platform expectations; [packaging](../../development/packaging.md) records the configured test bundles. Health checks expose OS support, disk encryption, disk space, backup age and clock state with explicit unknown/qualification limits.

## Alternatives

| Alternative | Why not chosen |
| --- | --- |
| Lab-operated cloud deployment | Requires hosted infrastructure and an internet-dependent clinical system of record; the accepted scope keeps data and operation on the laboratory computer |
| Hosted multi-tenant LIS (SaaS) | Contradicts "runs on one lab computer" and data portability (NFR-12); makes a service provider responsible for laboratory workflow availability |
| Background service plus browser or desktop client | Can receive while the OS is signed out, but adds platform-specific service installation and lifecycle management beyond the accepted one-process form |
| LAN client-server with several writing hosts | Unnecessary at the target scale. Revisit if more than one host must write or SQLite contention is measured |

## Consequences

- **Listening needs an open application and a signed-in OS session.**
  - Lab setup must keep the host computer powered, awake and signed in, with OS updates scheduled outside working hours.
  - Gaps from a reboot or closed app are covered by analyzer retransmission, durable receipts and expected-result reconciliation, never assumed away.
- **Single point of failure.** A disk failure, theft, ransomware or power loss affects the whole lab. Backup and recovery move to the center of the design:
  - encrypted local and off-PC copies, at least one immutable;
  - a recovery kit kept off the PC;
  - a rehearsed restore to a replacement computer;
  - a UPS ([deployment](../deployment.md), NFR-04, NFR-20).
- **The lab's own administrator is the privileged actor.** Anyone with OS administrator or root rights can read or alter the database file. Tamper evidence therefore depends on anchors leaving the computer on a schedule ([ADR 0008](0008-tamper-evident-audit-and-immutable-recovery.md)).
- **No project SLA for local availability.** Availability depends on the lab's hardware, power and OS patching. Any future external-service objectives need separate acceptance and evidence.
- **Simpler concurrency.** The guard-row protocol for release versus invalidation is not needed while there is one writer. The tests in both commit orders still apply.
- **OS lifecycle becomes product scope on three platforms.**
  - Each release must name supported OS/webview versions and review their servicing status ([deployment baseline](../deployment.md#host-baseline)).
  - Every release is tested on each supported OS.
- **Each update is a validation event for the lab.** Releases must ship a change summary, risk classification and validation evidence, so that small labs can accept updates without a consultant each time.
- **Data stays with the lab.** Exit means copying a database file, artifacts and a documented schema, which keeps the lab's data portable (NFR-12).

## Acceptance evidence and revisit

The proposed operating rules are accepted when the technical maintainer and project owner approve them after the [required verification](0012-rust-tauri-desktop-application.md#acceptance-evidence). Milestone tests and platform qualification must demonstrate:

- the application receiving a synthetic ASTM session while at its sign-in screen;
- a power cut during a write without losing committed work;
- release rechecking gates inside one write transaction, in both commit orders;
- a label printed while nobody is signed in to the app;
- an offline day followed by catch-up of the cloud queues;
- an update approved, applied and rolled back by snapshot restore.

Revisit if:

- a design partner needs more than one writing host or more than about 4 concurrent users;
- listening only while the app is open causes missed results in practice;
- measured volumes exceed the single-writer budget;
- the project owner changes the product shape.
