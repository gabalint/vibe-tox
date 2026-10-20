# Order-to-result contract index

The maintained specification is the [order-to-result contract](../architecture/order-to-result-contract.md). This index preserves section references used by source code, migrations and acceptance tests. Exact schemas and IPC types live in migrations and generated bindings; laboratory decisions live in the [policy input register](../quality/laboratory-policy-inputs.md).

## Reference contracts

[Current contract](../architecture/order-to-result-contract.md).

## 0. Ground rules for this design

[Current contract](../architecture/order-to-result-contract.md).

## 1. Scope cut

[Current contract](../architecture/order-to-result-contract.md).

## 2. Per crate

[Current contract](../architecture/order-to-result-contract.md).

### 2.1 `lis-orders` (`ord_`)

[Orders](../architecture/order-to-result-contract.md#orders).

### 2.2 `lis-catalog` M2 additions

[Orders](../architecture/order-to-result-contract.md#orders).

### 2.3 `lis-specimens` (`spc_`)

[Specimens](../architecture/order-to-result-contract.md#specimens).

### 2.4 `lis-analytical` (`ana_`)

[Analytical work](../architecture/order-to-result-contract.md#analytical-work).

### 2.5 `lis-results` (`res_`)

[Result evidence and review](../architecture/order-to-result-contract.md#result-evidence-and-review).

## 3. Cross-module contracts

[Receipt and import](../architecture/order-to-result-contract.md#receipt-and-import).

### 3.1 What `lis-results` calls

[Receipt and import](../architecture/order-to-result-contract.md#receipt-and-import).

### 3.2 From ASTM receipt to import (consumer of `ifc.receipt.completed`)

[Receipt and import](../architecture/order-to-result-contract.md#receipt-and-import).

### 3.3 Instrument specimen IDs and containers

[Receipt and import](../architecture/order-to-result-contract.md#receipt-and-import).

### 3.4 Keeping QC rows out of patient observations

[Receipt and import](../architecture/order-to-result-contract.md#receipt-and-import).

### 3.5 Re-export semantics on the ASTM route

[Re-export outcomes](../architecture/order-to-result-contract.md#re-export-outcomes).

### 3.6 What M3's release must recheck (M2 stores it all)

[Result evidence and review](../architecture/order-to-result-contract.md#result-evidence-and-review); [release protocol](../architecture/reporting-contract.md#release-protocol).

## 4. M2 IPC contract (engine ⇄ UI)

[IPC conventions](implementation-plan.md#core-contracts).

## 5. Verification boundaries

[Verification and boundaries](../architecture/order-to-result-contract.md#verification-and-boundaries).

## 6. Risks and open questions for the laboratory

[Laboratory policy inputs and current acceptance boundaries](../quality/laboratory-policy-inputs.md#order-to-result).
