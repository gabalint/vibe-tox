# ADR 0010: Optional cloud services with per-lab isolation

Status: proposed, 2026-10-12. Owners: project owner, a future service provider, privacy/security lead and counsel.

This ADR proposes boundaries for optional external services. It does not authorize their implementation; the local application remains the current focus.

- **Depends on** [ADR 0009](0009-single-computer-local-lis.md).
- **Authorization boundary:** shared hosting requires acceptance of this decision review and separate implementation authorization under `AGENTS.md`.

## Context

Future services could provide backup, secure report delivery, EHR interfaces and clinic or patient portals. They must preserve the [local system-of-record boundary](../../project-scope.md#product-boundaries).

These services hold PHI for many laboratories, which raises four constraints:

- **HIPAA.** A service provider becomes a business associate of each participating lab for every service, including zero-knowledge encrypted backup. Its cloud host, fax and Direct providers become subcontractors ([regulatory landscape §15.1](../../research/regulatory-landscape.md#151-hipaa-role-of-each-hosted-service)).
- **Fraud and abuse.** A portal furnished to referring clinics raises fraud-and-abuse questions that require counsel review; its scope stays limited to the furnishing lab's ordering and results ([§15.5](../../research/regulatory-landscape.md#155-fraud-and-abuse-laboratory-furnished-clinic-portals)).
- **Tenancy.** Shared hosting requires explicit authorization, a tenant threat model and negative isolation tests. That evidence is not established by this proposal.
- **Data location.** Texas requires electronic health records, including cloud-stored copies, to stay in the United States from 2026-01-01 ([§15.10](../../research/regulatory-landscape.md#1510-state-overlays-that-change-with-hosted-services)).

## Proposal

**1. Services are optional and separable.** The LIS works fully without them ([ADR 0009](0009-single-computer-local-lis.md)). A laboratory may use any subset. Each service has a local equivalent in the application. The equivalent may be manual or need the lab's own infrastructure:

| Service | What the hosted service adds | Local equivalent |
| --- | --- | --- |
| Backup | Offsite immutable vault, monitoring, restore verification | Encrypted backup to a lab-owned target |
| Report delivery | Portal, clinic print agent, cloud fax, Direct, per-channel evidence | Print, PDF and the lab's own channels |
| EHR interfaces | Hosted endpoint, monitoring and replay | Lab-run HL7 listener or file exchange |
| Client portal | Hosted clinic ordering and results | Lab staff enter faxed and paper orders |
| Patient portal (later) | Hosted patient access | Manual access and EHI export |

**2. Local authority.**

- The PC connects outbound only, over HTTPS with a per-device certificate. The private key is held in the OS key store, hardware-backed and non-exportable where available ([ADR 0012](0012-rust-tauri-desktop-application.md)).
- The cloud never creates or edits clinical facts. It forwards requests: portal orders, add-on and cancellation requests, inbound EHR orders.
- The LIS returns a durable receipt, then an application decision. A portal order is shown as "received by portal", then "received by lab", then "accepted" or "rejected".
- Both sides use durable outboxes with idempotency keys and per-recipient ordering of report revisions.
- The cloud accepts traffic only from the currently enrolled device. Re-enrollment after a restore revokes the old certificate.

**3. Data minimization per service.** The cloud holds only:

- backup ciphertext;
- the clinic catalog subset;
- pending orders until the LIS acknowledges them;
- released report versions and their supersession status, for authorized recipients;
- in-flight interface messages;
- delivery and access logs;
- digest-only audit anchors.

It never holds unreleased results, QC data, worklists or the full patient index. Retention per class is set with the lab and counsel. Whether cloud copies belong to the lab's designated record set is Q-15.

**4. Tenancy.**

- **Per-lab data.** One PostgreSQL database per laboratory on a shared cluster, with per-lab object prefixes and encryption keys.
- **Control plane.** A shared control-plane database holds tenants, devices and users, and no PHI.
- **Clinic logins.** Clinic users have one global login with authorizations held per lab. A clinic that orders from two labs sees two separate, lab-scoped contexts.
- **Rejected default.** Pooled shared tables with row-level security are not the default. Owners bypass row security unless it is forced ([ENG-05](../../research/sources-engineering.md)), and cloud vendors call that model complex to test ([ENG-125](../../research/sources-engineering.md#single-computer-technology-sources)).
- **Revisit.** Reassess at several hundred tenants.

**5. Business-associate operation.**

- **Agreements.** One master BAA per lab, with a schedule of services and data. A public subcontractor list.
- **Location.** US-only data location and US-only support access for every PHI-bearing service and subcontractor.
- **Telemetry.** Telemetry, crash reports and support bundles are PHI-free by default. Otherwise the free software would create a business-associate relationship with every install.
- **Incidents.** Incident notice to the lab well inside the 60-day maximum.

**6. Portal boundary.**

- **Scope.** Clinic-facing functions cover only ordering from, and results of, the furnishing lab: orders, requisitions and labels, status, results and reports, supply requests and order messages.
- **Excluded.** No scheduling, e-prescribing, charting, analytics on the clinic's business, or other labs' results.
- **No referral targeting.** No feature lets a lab target or withhold access by a clinic's referral value.
- **Clinic-side costs.** Whether a lab or a service provider may ever pay a clinic's EHR-vendor fees is a counsel decision (Q-32).
- **Records.** The LIS keeps the record of services furnished to each clinic (REQ-BIL-003, REG-032).

**7. Portability and exit.**

- **Relay protocol.** The relay protocol and payload schemas are published.
- **Exit.** A lab can export everything the cloud holds for it and keep operating locally.
- **Local safety and portability.** The application keeps backup, restore, export, manual delivery and lab-run interfaces, with no hosted service required.
- **Server code.** The license of the cloud-service server code is Q-11.

**8. Delivery evidence and acknowledgements.**

- **Delivery wording.** Every delivery record names the channel and its evidence level. A bare "delivered" is never shown.
- **Interface acknowledgements.** The cloud interface endpoint returns only a commit-level acknowledgement. The LIS application decision follows separately ([ADR 0007](0007-integration-edge-and-interface-engines.md); [cloud-services research §6](../../research/cloud-services.md#6-delivery-channel-confirmation)).

## Alternatives

| Alternative | Assessment |
| --- | --- |
| A separate stack per lab | Strong isolation with more independent operational infrastructure. Qualify backup, keys, recovery and support for each lab. |
| Pooled tables with row-level security | Shares storage infrastructure; isolation depends on every query and role. Revisit at several hundred tenants |
| No hosted services; local application only | The laboratory supplies any external channels it needs; local workflows, backup and export remain available |
| Connect third-party services (interface networks, fax, backup) | Possible for individual channels under the applicable BAA. Not a substitute for the lab-scoped portal or the local-authority protocol |
| Proprietary, undocumented relay protocol | Rejected: contradicts the portability and exit rules (item 7) |

## Consequences

- **A future service provider would run regulated infrastructure.** It would need:
  - on-call coverage;
  - a security program;
  - subcontractor BAAs;
  - incident handling across many covered entities;
  - insurance;
  - a documented support boundary.
- **Shared-tenant hosting needs this ADR accepted first.** It is permitted only in the database-per-lab form, after the evidence below.
- **Cloud services are part of each lab's CLIA reporting chain.** The lab needs per-destination evidence and periodic checks that results arrive intact ([§15.4](../../research/regulatory-landscape.md#154-clia-with-one-pc-as-the-laboratorys-record-system)).
- **Referral-related boundaries need counsel review.** Review any arrangement that furnishes portal or interface access to referring clinics; preserve the limited-use boundary and prohibit referral-value access controls.
- **Hosting selection.** Q-09 concerns a hosted environment for PHI-bearing services; the LIS remains local. No hosting provider has been selected.

## Acceptance evidence and revisit

Accept after all of the following:

- counsel reviews the portal boundary and BAA template (Q-27, Q-32);
- a tenant threat model and negative isolation tests on every data path: API, workers, exports, backups, logs and support tools;
- per-lab export, deletion and restore are rehearsed;
- the device enrollment and revocation drill passes;
- an offline-queue and correction-propagation scenario passes;
- a named service provider with BAAs from its hosting, storage and delivery subcontractors.

Revisit:

- when tenant count, operational needs or a lab's requirements demand a different isolation model;
- if counsel rejects the structure of a per-lab portal furnished free to clinics;
- if the project owner changes the scope of external services.
