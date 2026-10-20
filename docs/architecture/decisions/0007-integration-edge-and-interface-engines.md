# ADR 0007: LIS-owned canonical contracts with an optional external interface engine

Status: proposed, 2026-10-12. Decision owners: technical maintainer and interface owner, with laboratory/quality review of mappings; a service provider would co-own any hosted edge. Depends on Q-05, Q-06, [ADR 0009](0009-single-computer-local-lis.md), [ADR 0010](0010-optional-cloud-services.md) and [ADR 0012](0012-rust-tauri-desktop-application.md).

## Context

A toxicology laboratory typically connects several ordering clinics and EHR products. Most expect HL7 v2.5.1 order/result exchange with partner-specific variation. The current US laboratory guides are LOI and LRI Edition 5, both STU. eDOS has been retired in favor of a FHIR Order Catalog direction, and ASTP's standards pages still cite older LRI/LOI releases. Partners will therefore cite different editions. [DOM-009](../../research/sources-domain.md), [ENG-27–29](../../research/sources-engineering.md)

The LIS runs on one host computer in each laboratory, which is the system of record ([ADR 0009](0009-single-computer-local-lis.md)). The host opens no inbound internet port. EHR interfaces are either run by the laboratory itself or handled by a separately authorized service provider, which relays to the host over an outbound-only link and is a business associate of the lab ([ADR 0010](0010-optional-cloud-services.md)). The target office and small labs rarely have interface analysts; that is an assumption to confirm in discovery.

No interface engine or HL7 parsing library is selected or bundled. A future integration component needs a maintained released version, confirmed advisory-fix status, license compatibility, source-payload preservation and per-partner qualification. A version frozen only because its license remains available is not an adequate maintenance plan. Component assessment belongs to the scoped interface work; the clinical contract below remains independent of product selection.

The design puts durable inbox/outbox, semantic acceptance, reconciliation and correction ordering inside the LIS ([integrations](../integrations.md)). Those are clinical-safety properties. They cannot be delegated to a transport component's message store or acknowledgement, whether that component runs in the lab or in a hosted service.

## Proposal

1. **LIS owns the clinical contract.** It owns the following:
   - durable receipt
   - deduplication identity
   - semantic acceptance
   - domain commands
   - destination revision chains
   - reconciliation state

   An edge acknowledgement, engine ACK or message-store entry is never LIS acceptance or delivery evidence. Transport/edge acknowledgement and LIS application acceptance are distinct. The LIS's application outcome (accept, reject or error) must reach the partner in one of two ways:

   - passed through the edge, or
   - sent as a separate application acknowledgement or order response, for example ORL^O22.

   An edge configured to auto-acknowledge at the application level does not qualify, because it can hide an LIS rejection.
2. **Publish one canonical profile per exchange.** Inbound orders use an LOI Edition 5–based HL7 v2.5.1 profile. Outbound results use an LRI Edition 5–based profile. FHIR R4/US Core (ENG-34) is a later P1 route. Each profile documents its optionality, code systems, status and correction semantics, plus synthetic conformance fixtures. The same profiles apply at every edge; partners that conform connect without transformation.
3. **The integration edge is one of the following, and replaceable:**
   - **Lab-run listener or file exchange on the host.** The host application's narrow MLLP listener, bound to the lab LAN and running while the application is open, or file exchange over SFTP that the host connects to outbound. Partners outside the lab network reach the listener only through a lab-controlled VPN.
   - **A hosted EHR interface service (optional).** An endpoint per clinic EHR in the lab's tenant, relaying to the host over the device-authenticated outbound link ([integrations](../integrations.md#relay-and-sync-protocol)). It runs under the lab's BAA, with US-only data and support access (REQ-SVC-006).
   - **A laboratory-run external engine (optional).** An engine the lab already operates and qualifies, placed in front of the host listener for partner variants.

   The open-source app ships no engine and depends on none for durability. A lab may change edge, for example from the cloud service back to its own listener, without changing the canonical contract or losing history.
4. **Every edge in use is a qualified clinical component.** This applies to a hosted service and to a lab-run engine:
   - Channel configuration and mapping code are versioned, reviewed configuration with clinical effect (REQ-INT-001/004). They are recorded in the interface compatibility record.
   - Its message store holds PHI. It falls under the installation's or tenant's retention, access, backup, encryption, audit and BAA/data-flow scope.
   - It is patched from a maintained source and runs a named *released* version whose fix status for known advisories is confirmed. Merged-but-unreleased fixes do not count.
   - It forwards the original partner payload bytes unchanged, alone or alongside any transformed message, with the partner's identifiers.
5. **Original bytes reach the host before acceptance** (REQ-INT-008). The host durably commits the original partner payload bytes and a receipt record before it issues the LIS receipt ID, and before any application decision. Edge retention is secondary and never the only copy of source evidence, so edge pruning cannot remove it. The cloud endpoint keeps the bytes until the host confirms durable receipt, plus a reconciliation window.
6. **The cloud endpoint's commit acknowledgement is never LIS acceptance.** The endpoint returns only a commit-level acknowledgement meaning "durably stored by the endpoint". The LIS decision follows separately, as ORL^O22 or the negotiated equivalent, after the host is online and has decided. Partner tolerance for an asynchronous application response is unknown and is tested per partner. A partner that needs a synchronous application ACK gets one only while the host is online within the partner's timeout; otherwise the endpoint returns an error, never an acceptance, and that partner falls back to the portal or manual route ([integrations](../integrations.md#cloud-ehr-interface-endpoint-acknowledgement-split)).
7. **Parsing boundary.** A future narrow HL7 v2 codec and MLLP framer must follow the [library policy](../../governance/library-policy.md), preserve original bytes and pass a conformance/malformed-input corpus. No parser or external engine may substitute for domain validation. Runtime and dependency choices remain part of the scoped interface decision.

## Alternatives

| Option | Benefit | Cost / risk |
| --- | --- | --- |
| Build all partner mapping in LIS code | One runtime; full test control | Every client onboarding needs developers and a software release, and each release is a lab-approved update. This duplicates engine features (channels, replay UI, monitoring) that interface analysts already use. |
| Embed or bundle an engine on the host | Transformation and monitoring tools | Adds another runtime, PHI store and administrative surface to maintain and qualify on every laboratory computer. |
| Require the lab's existing engine for every route | Reuses lab staff and skills | Excludes the target labs, most of which have no engine, and still needs a canonical LIS contract. |
| External engine at a future hosted edge | Shared channel, replay and monitoring tools without another local runtime | Runtime/store enter the provider's PHI scope; per-lab isolation, maintenance and fix status require evidence (REQ-SVC-002). |
| **Canonical contract + replaceable edge: host listener, optional hosted service, optional lab engine (proposed)** | Clinical semantics stay in one place; labs can run interfaces locally or choose an external service, and reuse an engine if they have one | Two places to configure a partner. End-to-end receipt and correlation must be designed across the extra hop, and the cloud edge must split commit from application acknowledgement. |

## Consequences

The order and result contracts in [integrations](../integrations.md) include correlation across the edge hop:

- the partner's message control ID
- the edge message ID: the engine's, where available, or the cloud endpoint's
- the cloud receipt ID, for relayed items
- the LIS receipt ID

Reconciliation compares partner, edge and LIS counts. An edge-side filter or transformer error is an interface exception, not a silent drop. The compatibility record names the edge (host listener, hosted service or engine), its product/version and channel version. Replays from the edge keep the original partner identifiers, so LIS deduplication still applies. Edge operations, such as user accounts, MFA and logs, follow [security](../security-and-privacy.md).

Further consequences of the single-computer scope:

- **The cloud endpoint is part of the lab's CLIA delivery chain.** The lab needs per-destination evidence and periodic intact-arrival checks for routes through it (REG-047).
- **Host downtime delays application responses.** Partners see commit acknowledgements while the host is offline; queue age is visible to the lab and the service provider.
- **Local route required before service dependence.** An accepted hosted interface must have a qualified lab-run alternative; the current analyzer/file workflows do not establish that future HL7 route (Q-30).

## Acceptance evidence and revisit

Before acceptance, the following evidence is needed:

- a design-partner inventory of partner profiles, the clinics' EHRs and any engine the lab already runs (Q-05)
- a synthetic conformance corpus for both canonical profiles, including partial, corrected, censored, duplicate, conflicting and out-of-order messages
- a demonstration that an edge-mediated route (engine or cloud endpoint) preserves original payloads in host storage, IDs and end-to-end receipt states through edge restart and replay
- a demonstration that an LIS application rejection reaches the partner through each edge
- for the cloud endpoint: an offline-host day with commit-only acknowledgements, followed by catch-up in which the LIS decisions reach the partner; device revocation fencing; negative tenant-isolation tests on the endpoint's message store
- per partner, a recorded test of how the EHR treats a commit-only acknowledgement and a later ORL^O22 rejection
- a license and maintenance review of any engine or library

Revisit when any of the following occurs:

- a FHIR-first partner base emerges
- an adopted interface component loses maintenance or a provider proposes changing the hosted edge
- most partner EHRs cannot accept an asynchronous application response
- a commercial engine becomes the lab's mandated standard
- the product shape or the cloud-services model changes
