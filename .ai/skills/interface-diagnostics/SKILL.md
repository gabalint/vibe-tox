---
name: interface-diagnostics
description: Specify or diagnose LIS instrument and partner exchanges, including identity mapping, payload semantics, acknowledgement, replay and reconciliation.
---

# Interface diagnostics

Read [integration architecture](../../../docs/architecture/integrations.md) and the relevant workflow/requirements. Identify sender/receiver, software and protocol versions, allowed program/use, licensed interface rights and the owner of each mapping.

Document transport, message/profile, source IDs, specimen/aliquot identity, analyte/method/version, value/comparator/unit/precision, status, timestamps and correction semantics. Separate transport acknowledgement, durable receipt, semantic acceptance, clinical review, report release and recipient acknowledgement.

Walk through duplicate payload, same-ID/different-payload conflict, partial batch, delayed correction, unknown mapping, locale/decimal change, clock skew, restarted host service, interrupted serial session and unavailable destination. For a route through optional hosted services, also separate the cloud endpoint's commit-level acknowledgement from the LIS application decision. Check offline queueing while the lab computer is off, stale cached copies after a correction, and each delivery channel's actual evidence level ([ADR 0010](../../../docs/architecture/decisions/0010-optional-cloud-services.md)). Identify durable retention, retry boundaries, quarantine state, reconciliation counters and permitted recovery actions by the laboratory or, for hosted routes, the service provider.

Use vendor-supported exports and permitted synthetic samples. Do not assume S3 compatibility, HL7, FHIR, CSV or ASTM naming proves conformance to a partner's actual profile. Do not interpret proprietary raw instrument data or claim a connector is supported without version-specific evidence.

Return a conceptual contract and acceptance scenarios linked to requirement IDs. Writing adapter code or sending a real message requires implementation/task authorization; this skill produces documentation.
