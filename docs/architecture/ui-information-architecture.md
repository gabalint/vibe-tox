# UI information architecture

This document describes the desktop application's current navigation and record homes. The [UI standards](../governance/ui-standards.md) and [page layout standard](../governance/page-layout-standard.md) govern shared interaction and presentation. Field meanings, permissions, action gates and recovery procedures belong in [built-in help](../help/README.md) and the [operator guide](../operations/README.md).

Record names follow the [domain model](domain-model.md); capacity and platform qualification remain in [nonfunctional requirements](../requirements/nonfunctional-requirements.md) and [release readiness](../quality/open-items-g1-g2.md). Proposed changes are confined to [open points](#open-points).

## Principles

1. **Each record type has one home.** Directories and queues open a record page or a side panel. Other views link to the owning view rather than duplicating its actions ([ENG-174](../research/sources-engineering.md#ui-interaction-and-information-architecture-sources-2026-10-13)).
2. **Relationships provide navigation.** Parent records expose their children, and record references link to the relevant page or panel.
3. **Work queues expose pending work.** Reference directories support configuration; the sign-in landing page is Work queues.
4. **The engine owns authority.** Read models provide record state and, where available, allowed actions. Capability-based UI visibility helps navigation but never replaces engine authorization or clinical gates.
5. **Bounded lists preserve reachability.** Continuation, explicit refresh and stable record IDs keep pending work reachable beyond the first page. Historical display sorting is not evidence of clinical chronology.

## Navigation

The navigation and route definitions are maintained in [`navigation.ts`](../../ui/src/app/navigation.ts), [`setupNavigation.ts`](../../ui/src/app/setupNavigation.ts) and [`routes.tsx`](../../ui/src/app/routes.tsx). Areas appear only when the current session can reach at least one screen. Every area starts collapsed; a collapsed area shows the total of its entries' queue counts. The navigation remembers which areas the person opened or closed on that computer, and going to a screen opens its area.

| Area | Screens in navigation order |
| --- | --- |
| No heading | Work queues |
| Front desk | Patients, Orders, Incomplete identity intake, Storage excursions, Container lookup |
| Bench | Worklist, Runs, Bench setup |
| Review | Result review, Result imports |
| Reports | Release queue, Delivery, Reports, Report templates |
| Billing | Billing follow-up, Billing work, Billing class mappings |
| Quality and audit | Audit trail, Audit anchors |
| Test catalog | Tests and panels, Methods, Analytes, Specimen types |
| Laboratory setup | Material configuration, Laboratory profile, Catalog review, Clients, Practitioners, Delivery destinations, Instruments, Users, Starter data |
| System | Host health, Backups, Recovery kit, Versions and updates, Operations settings, Media disposals, Exit export |

Record routes, such as the exact deviation page, are reached through their owning records or work items and need not have a separate navigation entry. Help is available from the application shell. The component gallery is available only in development builds.

## Record homes

A **panel** opens beside its directory or workspace; a **page** has its own record route. Sensitive record reads use the engine's audited-read path and load on explicit opening, as required by [Sensitive records](../governance/ui-standards.md#sensitive-records).

### Test catalog and laboratory setup

| Record | Home and view | Child records and actions |
| --- | --- | --- |
| Test or panel, method | Respective Test catalog directory → versioned page | Version list and selected version's components/content; draft and lifecycle actions |
| Specimen type | Test catalog › Specimen types → page | Versioned validity policy |
| Analyte | Test catalog › Analytes → panel | Code, name, LOINC and supplied drug class |
| Client | Laboratory setup › Clients → page | Factual corrections, active state, locations and delivery destinations |
| Client location | Client page | Add or correct location with attributable history |
| Practitioner | Laboratory setup › Practitioners → panel | Name, NPI, factual correction and active state; local Find field |
| Delivery destination | Laboratory setup › Delivery destinations or client page → panel | Verification, replacement and retirement; available verification evidence |
| Instrument | Laboratory setup › Instruments → panel | Connection, listener configuration and outcome of the last change |
| User | Laboratory setup › Users → panel | Roles, active state and permitted sign-in recovery actions |
| Laboratory profile | Laboratory setup › Laboratory profile → page | Versioned reporting and performing-laboratory facts |
| Material configuration | Laboratory setup › Material configuration → page | Label profiles and storage configuration |
| Catalog Preview | Laboratory setup › Catalog review → panel; PDF in report window | Generate, view, save and approve an exact reviewed catalog snapshot ([contract](../planning/starter-data-brief.md#catalog-preview-and-approval)) |
| Billing class mapping | Billing › Billing class mappings → versioned page | Mapping versions and entries |

Versioned record pages show the version in effect and any open draft in the header, with a compact Versions list. `?version=` selects a version; without it, the page shows the open draft, then the version in effect, then the newest available version. Directory read models expose `versionInEffect` and `draftVersion`. Approval authority and separation of duties follow [ADR 0015](decisions/0015-separation-of-duties-policy.md) and [ADR 0018](decisions/0018-remediation-authority-and-method-continuity.md).

### Front desk, bench, review, reports and billing

| Record or work item | Home and view | Relevant boundary |
| --- | --- | --- |
| Patient | Front desk › Patients → page | Order-entry or quality staff may read; patient/order creation and standing-authority actions retain their own permissions |
| Incomplete identity intake | Front desk › Incomplete identity intake | Retains uncertain DOB evidence for explicit resolution or decline; does not invent demographics or create an order |
| Order | Front desk › Orders → page | Supplied evidence revisions, cancellation decisions and billing context remain attributable |
| Accession and container | Order page, scan field or Container lookup → page | Connected intake preserves patient/order context; receiving, allocation, collection facts and label decisions are explicit |
| Run | Bench › Runs → page | Positions, QC and expected results remain linked to their exact run |
| Lot and instrument readiness | Bench › Bench setup → tab and panel | Lots expose withdrawal; instrument panels expose service status and approved result sources |
| Deviation | Work item or related record → `/quality/deviations/:deviationId` | Exact audited read independent of run scope |
| Specimen results | Review › Result review → page | Result lines form the technical-review workspace |
| Import attempt | Review › Result imports → page | Quarantine decisions operate on the selected retained attempt/item |
| Report and revision | Reports or Release queue → page | Frozen candidate, release and revision history remain distinct |
| Delivery obligation | Reports › Delivery → panel | Attempts and their channel/evidence level belong to the exact obligation |
| Billing evidence | Billing › Billing follow-up or order billing context | Financial evidence does not authorize clinical work |
| Billing derivation and handoff | Billing work → derivation or handoff page | Bulk handoff selection is explicit; individual outcomes belong to the handoff item |

[`itemPath.ts`](../../ui/src/features/workqueues/itemPath.ts) owns work-item routing. Deviations, preservation incidents, delivery obligations, billing patient/date work and handoff items retain their exact identities across list limits. Unknown item kinds do not get a guessed record link. The connected workflow is described in [daily operation](../operations/daily-operation.md).

### System

| Record | Home and view | Purpose |
| --- | --- | --- |
| Backup target, run and restore test | System › Backups → Targets, Runs or Restore tests tab → panel | State, retained backup sets and restore-test requests |
| Health check and application run | System › Host health or Health history → panel | Explicit attestation/acknowledgement and exact work-item links |
| Software version and update approval | System › Versions and updates → tab and panel | Review and approval records; signed distribution and updater remain release gates |
| Recovery kit, operations settings, media disposal and exit export | Corresponding System page | Follow the [operator procedures](../operations/README.md) for permissions, evidence and recovery |

## Shared components

Directories compose `Page`, `FindField`, `DataGrid` and, where applicable, `DetailPanel`. Record actions belong to the opened page/panel or its row menu. Checkbox selection is reserved for actual bulk operations. [`VersionedRecordPage`](../../ui/src/features/configuration/VersionedRecordPage.tsx) provides the catalog version interaction.

| Component | Responsibility |
| --- | --- |
| `DataGrid`, `RowActions` | Row opening, current-row indication, accessible per-row actions and explicit bulk selection |
| `DetailPanel` | Docked record view, close/focus return and responsive replacement of the list; open identity in `?open=` |
| `Tabs` | URL-addressable workspace sections |
| `FindField`, `RecordPicker` | Search a collection or choose one existing record; patient-identifying search criteria remain in memory |
| `PageHeader`, `RecordActions` | Record context, breadcrumb, status, navigation and applicable actions |
| `IdentityBanner` | Patient/specimen identity and relevant holds on clinical record views |
| `StatusBadge`, `Alert`, `Button` | State, blocking feedback and actions using the canonical UI tokens and tone meanings |

The component contracts and presentation rules are maintained in [UI standards](../governance/ui-standards.md) and [page layout standards](../governance/page-layout-standard.md); this map does not duplicate their styling specification.

## Open points

| Question | Current boundary and evidence |
| --- | --- |
| Practitioner-to-client relationships | Order entry offers practitioners independently of client. A maintained relationship and laboratory workflow are needed before narrowing by client. FHIR's practitioner-role model is supporting prior art, not an implemented feature ([DOM-102](../research/sources-domain.md#ui-information-architecture-sources-2026-10-13)). |
| Practitioner search at scale | The current engine lists practitioners and the UI filters them locally. A bounded engine query with explicit continuation needs design and workload evidence if that list approaches the NFR-33 capacity target. |
| NPI registry prefill | Outside current scope. A future NPPES lookup would require an explicit network/data-flow decision and staff confirmation of imported values ([DOM-103](../research/sources-domain.md#ui-information-architecture-sources-2026-10-13)). |
| Instruments in two areas | Connection administration and bench readiness have different permissions. Combining them needs a read-model and visibility decision; current navigation keeps both homes. |
| Collection capacity | Per-directory capacity requires laboratory counts and reference-host measurements. NFR-33 is a target; synthetic lists and catalog content do not establish a laboratory's actual workload. |
| Action metadata | Some views still derive visibility from capability and status fields rather than an `allowedActions` collection. Every engine command must continue enforcing the authoritative gate. |
