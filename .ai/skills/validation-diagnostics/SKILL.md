---
name: validation-diagnostics
description: Review LIS verification plans, requirements traceability, safety scenarios and release-readiness evidence without confusing planned tests with passed validation.
---

# Validation diagnostics

Read [validation strategy](../../../docs/quality/validation-plan.md), [requirements](../../../docs/requirements/functional-requirements.md), [compliance](../../../docs/requirements/compliance-matrix.md) and affected [hazards](../../../docs/planning/risk-register.md). Identify the stage: paper design, synthetic demo, shadow pilot or operational release.

Trace the changed requirement through hazard/control, design, scenario, expected result, evidence artifact and responsible reviewer. Find missing negative/concurrency/recovery cases where a happy-path check would conceal clinical or privacy risk. Evaluate test adequacy by observable behavior, not coverage percentage or matching implementation details.

For claimed completed checks, inspect actual records and environment/version/configuration. Label unexecuted scenarios as planned. Require laboratory acceptance for intended-use suitability and laboratory evidence for local restore targets and service-provider evidence for hosted-service targets; software tests alone do not establish either.

Return findings ranked by concrete consequence, with affected IDs, missing evidence and the gate they affect. For an independent review, derive cases from requirements before reading the author's expected test results. Do not require production approval for ordinary documentation fixes, and do not approve production on behalf of human owners.
