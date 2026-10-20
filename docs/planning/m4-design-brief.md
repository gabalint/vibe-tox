# Local operations contract index

The maintained specification is the [local operations contract](../architecture/operations-contract.md). This index preserves section references used by source code, migrations and acceptance tests. Exact schemas and IPC types live in migrations and generated bindings; laboratory decisions live in the [policy input register](../quality/laboratory-policy-inputs.md).

## 0. Decisions and ground rules

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

## 1. Scope cut

[Current contract](../architecture/operations-contract.md).

## 2. `lis-ops` (`ops_`) and additions to other crates

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

### 2.1 Layout and owners

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

### 2.2 Tables (every table STRICT, with typed IDs `Id<K>`)

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

### 2.3 State machines (state is stored only where a `state` column exists)

[Restore and reconciliation](../architecture/operations-contract.md#restore-and-reconciliation).

### 2.4 Commands (all gates read inside the command's transaction)

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

### 2.5 Queries

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces).

## 3. Backup and restore

[Backup contract](../architecture/operations-contract.md#backup-contract).

### 3.1 One backup run (job runner, blocking, inside `spawn_blocking`)

[Backup contract](../architecture/operations-contract.md#backup-contract).

### 3.2 `vtbk1` file format (the one home is `lis-ops/src/format.rs`; specified in `docs/architecture/backup-and-export-formats.md`)

[Encrypted backup files](../architecture/backup-and-export-formats.md#encrypted-backup-files-vtbk1).

### 3.3 Target layout, manifest and kit

[Backup and export formats](../architecture/backup-and-export-formats.md); [keys and recovery kit](../architecture/operations-contract.md#keys-and-recovery-kit).

### 3.4 Schedule, catch-up and retention

[Backup contract](../architecture/operations-contract.md#backup-contract).

### 3.5 Verification levels and health signals

[Verification expectations](../architecture/operations-contract.md#verification-expectations); [host health](../architecture/operations-contract.md#host-health-and-application-lifecycle).

### 3.6 Restore test (to scratch; never touches live data)

[Restore and reconciliation](../architecture/operations-contract.md#restore-and-reconciliation).

### 3.7 Restore onto a new installation (replacement computer or rebuilt host)

[Restore and reconciliation](../architecture/operations-contract.md#restore-and-reconciliation).

### 3.8 Recovery reconciliation (REQ-OPS-002)

[Restore and reconciliation](../architecture/operations-contract.md#restore-and-reconciliation).

### 3.9 Key custody

[Keys and recovery kit](../architecture/operations-contract.md#keys-and-recovery-kit).

## 4. Audit sealing and anchors (ADR 0008, NFR-18)

[Audit seals and anchors](../architecture/operations-contract.md#audit-seals-and-anchors).

## 5. Exit export (REQ-OPS-010, NFR-12)

[Exit export](../architecture/operations-contract.md#exit-export).

## 6. Host health, keep-awake, close warning, startup

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

### 6.1 Checks (one table in `lis-ops/src/health/checks.rs`; probes in `host/`, I/O separated from evaluation)

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

### 6.2 Keep-awake (REQ-OPS-013)

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

### 6.3 Close and quit warning (REQ-OPS-013)

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

### 6.4 Startup sequence

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

## 7. Update approval records (REQ-OPS-006, NFR-24; no signed channel until Q-19)

[Update records](../architecture/operations-contract.md#update-records).

## 8. Settings

[Host health and application lifecycle](../architecture/operations-contract.md#host-health-and-application-lifecycle).

## 9. IPC contract (M2 §4 conventions; (A) = audited read; "dialog" = the adapter opens it in Rust and returns `null` when it is cancelled)

[Ownership and interfaces](../architecture/operations-contract.md#ownership-and-interfaces); [operations IPC index](m4-contract.md).

## 10. Verification boundaries

[Verification expectations](../architecture/operations-contract.md#verification-expectations).

#### Tests that need real hosts

[Real host qualification](../architecture/operations-contract.md#real-host-qualification).

## 11. Risks and open questions

[Laboratory policy inputs and current acceptance boundaries](../quality/laboratory-policy-inputs.md#operations).
