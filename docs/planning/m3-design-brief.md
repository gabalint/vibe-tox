# Reporting and handoff contract index

The maintained specification is the [reporting and handoff contract](../architecture/reporting-contract.md). This index preserves section references used by source code, migrations and acceptance tests. Exact schemas and IPC types live in migrations and generated bindings; laboratory decisions live in the [policy input register](../quality/laboratory-policy-inputs.md).

## 0. Ground rules (additions to M2 §0)

[Report record and snapshot](../architecture/reporting-contract.md#report-record-and-snapshot).

## 1. Scope cut

[Current contract](../architecture/reporting-contract.md).

## 2. Per crate

[Current contract](../architecture/reporting-contract.md).

### 2.1 `lis-reporting` (`rpt_`)

[Report record and snapshot](../architecture/reporting-contract.md#report-record-and-snapshot).

### 2.2 `lis-billing` (`bil_`)

[Billing derivation and handoff](../architecture/reporting-contract.md#billing-derivation-and-handoff).

### 2.3 Dependencies on existing crates

[Release protocol](../architecture/reporting-contract.md#release-protocol).

### 2.4 Results contract that M3 consumes

[Release protocol](../architecture/reporting-contract.md#release-protocol).

## 3. Release protocol

[Release protocol](../architecture/reporting-contract.md#release-protocol).

### 3.1 Steps

[Release protocol](../architecture/reporting-contract.md#release-protocol).

### 3.2 Snapshot (content digest = SHA-256 of the canonical JSON; signatory and time live on the candidate row)

[Report record and snapshot](../architecture/reporting-contract.md#report-record-and-snapshot).

### 3.3 Report data: `tox-report` v2 (`ToxReportData`, camelCase, strings or null only)

[Report record and snapshot](../architecture/reporting-contract.md#report-record-and-snapshot).

### 3.4 Final transaction (`release_report`), in order

[Release protocol](../architecture/reporting-contract.md#release-protocol).

### 3.5 Writes in the final transaction

[Release protocol](../architecture/reporting-contract.md#release-protocol).

### 3.6 Test plan: both commit orders (`lis-engine/tests/scenarios/m3_release_races.rs`, outcomes from this table)

[Release race expectations](../architecture/reporting-contract.md#release-race-expectations).

## 4. Corrections, withdrawal, delivery and verification

[Corrections withdrawal and preservation](../architecture/reporting-contract.md#corrections-withdrawal-and-preservation).

## 5. Billing

[Billing derivation and handoff](../architecture/reporting-contract.md#billing-derivation-and-handoff).

## 6. Work queues (REQ-OPS-004)

[Work queues and verification](../architecture/reporting-contract.md#work-queues-and-verification).

## 7. IPC contract (M2 §4 conventions; (A) = audited read)

[IPC conventions](implementation-plan.md#core-contracts); [release protocol](../architecture/reporting-contract.md#release-protocol).

## 8. Verification boundaries

[Work queues and verification](../architecture/reporting-contract.md#work-queues-and-verification).

## 9. Risks and open questions

[Laboratory policy inputs and current acceptance boundaries](../quality/laboratory-policy-inputs.md#reporting-and-handoff).
