# Research method and evidence index

These documents provide evidence and proposals for the [product scope](../project-scope.md). They do not establish clinical validation, legal clearance or a supported deployment.

## Evidence rules

Use primary sources for law, standards, platform behavior and technical claims. Record stable source ID, publisher/title, direct URL, checked date, publication/effective date when relevant, supported claim, applicability and limitations. A vendor page establishes advertised functionality, not qualified behavior. Search snippets are discovery aids; identify inaccessible underlying sources explicitly.

Distinguish requirements, conditional requirements, guidance, vendor claims, project proposals and unknowns. Preserve useful contradictory evidence and explain uncertainty. Do not reproduce restricted accreditation text, proprietary manuals or licensed terminology without permission. Use synthetic examples only.

## Research and source ledgers

| Area | Synthesis | Evidence |
| --- | --- | --- |
| Laboratory practice | [Laboratory practices](laboratory-practices.md) | [Domain sources](sources-domain.md) |
| Regulation | [Regulatory landscape](regulatory-landscape.md) | [Regulatory sources](sources-regulatory.md) |
| Billing intake | [Billing evidence and handoff](../operations/billing-evidence-and-handoff.md) | Domain and regulatory source ledgers |
| Patient reporting | [Report content and disclosure boundaries](patient-reporting.md) | [Patient-reporting sources](sources-regulatory.md#patient-reporting-sources) |
| Desktop platforms | [Platform evidence and qualification](desktop-application-stack.md) | [Engineering sources](sources-engineering.md) |
| Local storage and recovery | [Local storage, recovery and host evidence](single-pc-technology.md); implemented behavior and qualification limits | Engineering sources |
| Analyzer result files | [LC-MS/MS result exports](lcms-result-exports.md) | Domain and engineering source ledgers |
| Authentication | [Security-key research](security-key-authentication.md); implementation and remaining qualification limits | Engineering and regulatory source ledgers |
| Future external services | [External-service boundaries and interoperability](cloud-services.md); proposals outside current implementation scope | Domain, engineering and regulatory source ledgers |

Keep implementation guidance in the operator/developer guides and accepted choices in the ADRs; research supplies evidence, not a parallel specification. Surveys of options that current decisions exclude and commercial product planning do not belong in the evidence collection. Primary-source attribution covers instruments, laboratory practice, interoperability software and dependency licenses.

Source IDs are stable; reserved IDs supply no evidence. Technical interface-engine and immutable-storage sources are in the [engineering ledger](sources-engineering.md#integration-and-storage-sources).

## Refresh triggers

| Trigger | Evidence to refresh |
| --- | --- |
| Accepting a technical choice or release | Decisive platform documentation, dependency versions, licenses and security advisories |
| Qualifying a host OS | Vendor support status, web-engine behavior, key storage, printing and supported hardware |
| Qualifying an analyzer or interface | Exact software/firmware version, message/file contract, acknowledgement and replay behavior |
| Planning an operational pilot | Current law and effective dates, state and accreditation applicability, lab policies and unresolved compliance gaps |
| Proposing an external service | Data inventory, lawful agreements, isolation, retention, recovery, support access and exit |

No recurring research schedule is established. Unresolved decisions belong in [open questions](../planning/open-questions.md); hazards belong in the [risk register](../planning/risk-register.md).
