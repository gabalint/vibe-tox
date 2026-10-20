# Standard Catalog, catalog review and demo-data contract

Status: **implemented** under REQ-OPS-014–016. Bundled clinical content remains unreviewed until its named clinical review is recorded (Q-02/Q-39). Application configuration approval follows ADR 0018 and the configured separation-of-duties policy; loading or approving records does not establish laboratory validation.

## Decisions

| Concern | Current contract | Status |
| --- | --- | --- |
| Bundled data | Versioned JSON provides draft Standard Catalog content and separately marked fictitious demo parties/patients. | Accepted |
| Demo isolation | A demo laboratory has its own data folder, installation identity and key-store entry; it cannot be promoted or restored into a real installation. | Implemented isolation; separate-demo product assumption still awaits Q-39 confirmation |
| Clinical content | Urine and oral-fluid definitions; attributable published clinical starting cutoffs where suitable evidence exists; unsupported cutoffs and laboratory assay limits remain blank. | Accepted scope; laboratory review/validation required |
| Terminology | Pinned LOINC codes and display names retain Regenstrief's notice. No CPT content or payer rules are bundled. | Accepted |
| Demo identity | Coined surnames and conspicuously synthetic identifiers; no staff accounts or sign-in shortcuts. | Accepted |
| Catalog review | Catalog Preview and Approve reviewed catalog are ongoing Laboratory setup functions independent of starter loading. Approval binds one stored preview and its exact draft revisions, with fresh action confirmation and configured separation policy. | Accepted |
| First-run demo preparation | Optional whole-dataset loading is checked by default in demo setup; persisted preparation completes after normal authentication, atomically and retryably. No loaded catalog item gains clinical approval. | Implemented |

Operator steps and current authentication choices are in [installation and first start](../operations/installation-and-first-start.md#choose-a-demo-or-an-empty-laboratory).

## Summary

1. **Two loads, one review.**
   - **Load Standard Catalog** brings *definitions* into the laboratory's database as drafts: specimen types, analytes, methods, tests and panels.
   - **Load Demo Data** brings *fictitious parties and patients*.
   - **Catalog Preview** prints everything for checking. **Approve reviewed catalog** then approves exactly the printed drafts, under the laboratory's approval policy. This workflow applies to manually entered drafts as well as loaded ones.
2. **Nothing loaded can be removed.** The schema makes catalog codes, patients and orders permanent: triggers refuse updates and deletes, and no command discards a draft. The manual catalog loader shows a selection, summary and disclaimer before writing. First-run demo setup instead offers the complete bundled dataset with an explicit option and draft/synthetic-data explanation. Demo data stays in a separate demo laboratory, which can be reset as a whole.
3. **Every value has a source comparison.** The report labels matches to published or demo values in the current shipped file, missing values, and values that differ from the current starter. Matching by code and value does not reconstruct historical provenance: an unchanged older catalog value can differ after an app update. Saved previews remain unchanged.
4. **Instruments are not catalog content.** Analyzer connections, result sources and test-code maps are specific to each site, and vendor interface specifications are often licensed manuals. Analyzer interfaces are configured for each laboratory during setup.
5. **No CPT content.** LOINC codes ship with Regenstrief's notice; CPT content is excluded ([licensing and terminology](#licensing-and-terminology)).

## What a laboratory needs before its first order

This inventory describes the configuration needed before a first order. "Catalog" means the Standard Catalog file contains it. "Demo" means the demo data file contains it. "Lab" means the laboratory supplies its own.

| Data | Where it lives | Catalog | Demo | Lab | Notes |
| --- | --- | --- | --- | --- | --- |
| Specimen types | `cat_specimen_types` (code, name) | Urine, oral fluid | — | Confirms | Codes are permanent |
| Analytes | `cat_analytes` (code, name, LOINC code, drug class code) | Yes | — | Adds local ones | Class-level screening analytes stay distinct from molecules ([laboratory practices](../research/laboratory-practices.md)) |
| Methods | `cat_methods`, `cat_method_versions`: specimen type, presumptive or definitive, qualitative or quantitative, threshold rule and rounding, and analytes with unit, cutoff, LOD, LOQ, reportable range and display scale | Drafts, with published cutoffs where they exist | Demo values for the blanks | Fills and confirms every limit | Approval follows the separation-of-duties policy; permitted self-approval requires confirmation |
| Tests and panels | `cat_orderables`, versions, components with purpose (analytical or validity), component analytes | Drafts | — | Approves; adds its own | See [content rules](#content-of-version-1) |
| Specimen validity policy | `cat_validity_policy_versions`, categories with an action (none, qualifier, hold, recollect, comment) | No (Q-24) | — | Yes | Validity criteria and actions are laboratory policy |
| Analyzer, connection, result source, test codes | `ifc_instruments`, `ana_source_registrations`, `ana_source_test_codes` | No | No | Yes | Site-specific; configured during interface setup |
| Lots | `ana_lots`: reagent, calibrator, control, internal standard | No | No | Yes | |
| Storage locations | `spc_locations` | No | No | Yes | |
| Label printer | `spc_printers` (TCP or CUPS) | No | No | Yes | Network printers need a private address (Q-38) |
| Laboratory profile | `idn_laboratory_profiles`: name, address, CLIA ID, director, phone, time zone, billing NPI, tax ID | No | Fictitious, for the demo laboratory | Yes | |
| Users and roles | `auth_users`, `auth_role_grants` | No | No | Yes | Catalog approval needs `approve_configuration` and follows the laboratory's separation-of-duties policy |
| Clients and locations | `idn_client_organizations`, `idn_client_locations` (code, name) | No | Yes | Yes | |
| Practitioners | `idn_practitioners` (NPI with check digit, names) | No | Yes | Yes | |
| Delivery destinations | `rpt_destinations` (fax, email, mail, courier, pickup) | No | Yes | Yes | |
| Patients | `idn_subjects`, identity assertions, identifiers with assigning authority | No | Yes | — | |
| Billing class mapping | `bil_class_mapping_versions`, entries (analyte to class) | No | No | Billing owner (REG-031) | CPT's drug class list is AMA content |
| Backup target | `ops_targets` | No | No | Yes | |

Discrepancy codes, abnormal flags and the other enumerations are fixed in the product. Rejection and hold reasons are free text, so there are no configurable reason lists to seed.

The [domain model](../architecture/domain-model.md) also proposes records that are not built yet: medication mappings, interpretive rule sets, reporting policies with display grouping, order policies, client order profiles and payer code sets. The product ships no content for any of them by design.

## Standard Catalog

### Content of version 1

| Group | Content |
| --- | --- |
| Specimen types | Urine and oral fluid. Codes and provenance are recorded in the shipped file and its content notes. |
| Presumptive screens | One immunoassay method per specimen type: presumptive and qualitative, with class-level analytes. Examples: amphetamines, barbiturates, benzodiazepines, buprenorphine, cannabinoids, cocaine metabolite, methadone metabolite (EDDP), fentanyl, opiates, oxycodone, phencyclidine, tramadol and, for urine, ethyl glucuronide. The exact shipped menu and each entry's source are recorded in the versioned catalog file. |
| Definitive testing | One LC-MS/MS method per drug class and specimen type (opioids, fentanyl and analogues, benzodiazepines, stimulants, and the other classes in the shipped file), each with parent drugs and metabolites in ng/mL |
| Urine specimen validity | Creatinine, specific gravity, pH, oxidants and nitrite, as one method used by validity components ([DOM-002](../research/sources-domain.md)) |
| Tests and panels | One test per definitive drug class and specimen type, so clinics can order per patient. One screen panel per specimen type. One urine validity test. |

**Content rules:**

- **Analyte entries.** Each analyte entry carries:
  - a code and a name;
  - the LOINC code with its LOINC display name;
  - a drug class code from the project's own pharmacological grouping, not copied from CPT;
  - the source IDs that support the entry.

  The menu sources are the ADLM pain-management guideline and the ASAM materials (DOM-024, DOM-026). Neither is a cutoff or panel source.
- **Cutoffs.** Supply researched clinical starting values, including values published by clinical laboratories, for the matching analyte, specimen and analytical technique. Compare independent laboratories where possible, record disagreements, and retain free/total-drug and collection-device qualifications. A published laboratory value is evidence of that laboratory's practice, not proof of the nationally most common cutoff. Federal workplace values are not the clinical default. Leave a value blank only when suitable evidence is missing or the field cannot express the needed rule (for example a specimen-validity range). Each populated cutoff cites its source and still requires laboratory review. See the [content notes](../../crates/lis-catalog/standard/content-notes.md).
- **Analytes are specific to one specimen type.** A LOINC code names an observation in one specimen by one method, and analyte rows cannot be edited once created. The catalog therefore carries `UR-MORPH` and `OF-MORPH` as separate analytes, each with its own LOINC code.
- **Other limits** stay blank: limit of detection, limit of quantitation, reportable range and display scale. They belong to each laboratory's validated assay.
- **Required fields with a starter value.** The schema requires a threshold rule and a threshold rounding. Drafts get `at_or_above` and `before_rounding`, and the report marks both as "starter value, confirm".
- **No panels sized to billing tiers**, and no default bundling of presumptive and definitive testing. Panels follow clinical drug classes. This follows INV-13 and the OIG and DOJ findings on comprehensive panels (DOM-038, DOM-039, SR-049).
- **Disclaimer.** The load dialog, the Catalog Preview header and every catalog screen show this while loaded starter drafts remain unapproved: "Starter content: check every value before use. It is not validated for your laboratory and not ready for patient testing." The app-wide starter catalog notice has a close button. Dismissal lasts across navigation until the signed-in shell unmounts (lock, sign-out or app restart); it does not approve drafts or change catalog review status.

### Selection and implied selection

The load screen shows two trees:

- **Tests and panels**, each with its components;
- **Methods**, each with its specimen type and analytes.

Selection rules:

- **Implied items.** Choosing a panel or test selects its methods. Choosing a method selects its specimen type and analytes. Implied items show as checked and locked, with "Needed by" followed by the parent's name. Clearing the parent releases them.
- **Removing an analyte** from a chosen method is allowed. The method is loaded without it, and panels that used it lose it too.
- **Codes already in use:**
  - an item whose code already exists with the same name shows "Already in your catalog" and is not loaded again;
  - an item whose code exists with a different name shows "Code in use" and cannot be loaded, because codes are permanent.
- **Before writing**, a summary lists the counts per kind and shows the disclaimer, which the user must acknowledge.
- **After loading**, the screen links to Laboratory setup › Catalog review, where the laboratory generates a Catalog Preview and approves the reviewed drafts.

## Catalog Preview and approval

### Catalog Preview report

- **What it shows.** A PDF listing the whole catalog, whether it was entered manually or loaded; no starter-data load is required:
  - specimen types;
  - analytes with LOINC code and name and drug class;
  - each method's draft and current versions, with every analyte's unit, cutoff, limits, reportable range, display scale, threshold rule and rounding;
  - tests and panels with their components and component analytes;
  - validity policies.
- **Value sources.** Every value shows its source label from the [summary](#summary): published, blank, demo value or changed by the laboratory.
- **Gaps.** Blank cutoffs and starter values are highlighted, and a first page lists them.
- **Identity.** The report has an ID, the generation time, the generating user and a digest of its content. It lists each draft it covers with its revision.
- **Storage.** It is rendered by `lis-report` (Typst) and stored as a content-addressed artifact, so the copy the manager checked can be produced later. Generating it is audited.

### Approve reviewed catalog

- **Input.** The user chooses a Catalog Preview report and confirms.
- **Rights and checks:**
  - it needs `approve_configuration` and re-authentication;
  - the user must not be the author of any draft in the report (the two-person rule), unless the laboratory's separation-of-duties policy allows self-approval of configuration and the user confirms it ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)).
- **Refusals.** It is refused, naming the records, if:
  - any listed draft changed after the report was generated;
  - a new draft has appeared since;
  - the user authored one of the drafts and the laboratory's policy does not permit the self-approval.

  Changed or added drafts require a new report and review. A policy refusal requires an eligible approver.
- **Blank cutoffs.** If any analyte has a blank cutoff, the confirmation lists them. The user must confirm each as not applicable or go back and fill it, as REQ-ANA-009 requires.
- **Effect.** In one transaction it approves validity policies, then method versions, then test and panel versions. It calls the existing approval functions, so each record gets its usual audit event and the triggers' audit checks hold. One review record holds the report ID and digest, the reviewer and the time.
- **Afterwards.** The disclaimer banner clears automatically when no loaded draft remains; a person may also dismiss the notice for the current signed-in session as described above. Later edits create new drafts, which need a new report or single approvals.

### Draft method references

`check_manifest` (`crates/lis-catalog/src/commands/orderables.rs`) lets a test or panel draft reference a method version that is current approved **or still a draft**. A freshly loaded catalog can therefore hold test and panel drafts and be approved in one review cycle, rather than needing a first review to approve its methods. Approval rechecks that every component is current approved (`approve_orderable_version`, REQ-ANA-009), so nothing unapproved can be ordered. A method draft that a test or panel draft uses cannot drop an analyte the test or panel needs; the foreign key refuses it, and the error must say so.

## Demo data

### Where it loads

The demo data file ships with the app and loads only into the separate **demo laboratory** (Q-39):

- **Entry and storage.** Before a real laboratory has an account, its setup screen offers "Explore the demo laboratory". Confirmation restarts into its own data folder (for example `<app data>/demo/`) with its own installation ID and OS key-store entry; unsaved account fields from the original screen are not carried over.
- **Markings.** A permanent "DEMO LABORATORY — not for patient testing" banner, and a DEMO watermark on every report and label.
- **Reset.** "Reset the demo laboratory", confirmed with "Delete the demo data", removes its accounts and data and starts an empty demo again. "Leave the demo laboratory" returns to the laboratory's installation while retaining the demo.
- **No promotion.** A demo installation is marked in `ops_installation` and cannot become a real one. Restoring a demo backup into a real installation is refused ([deployment](../architecture/deployment.md)).
- **Sign-in.** Use normal first-account setup and the supported authentication route in the operator guide. With automatic demo loading selected, the account receives every role and the existing one-person separation policy; each self-approval still needs confirmation. With loading unchecked, the usual "Several people" or "Only me: I run this laboratory alone" choice applies ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)).
- **Automatic preparation.** "Load Standard Catalog and demo data" is checked by default only in the demo form. Account creation saves that request with its owner; an unlocked authenticated session completes it before the workspace opens. The catalog drafts, demo profile and parties, supported demo cutoff fills and completion marker commit together. Failure leaves preparation pending and offers "Try again"; interruption resumes after normal sign-in. Repeating a completed request creates no additional records. The engine rejects preparation in a real installation or completion by another account.
- **Review still applies.** Automatic loading creates the full catalog as drafts, with no clinical approval, extra staff, analyzers, lots, orders or results. An evaluator can inspect the prepared directory and catalog immediately; a practice order still needs reviewed, approved orderables. The pending screen states this next step.

The separate laboratory exists because loaded records cannot be removed. Fictional patients in a production record have caused patient-safety events (DOM-111). Wrong-patient errors appear in 15% of health IT events (DOM-113). Testing in production needs controls on test patients (DOM-112).

### Contents

The loader creates the data through the real engine commands, preserving authorization and audit; it does not insert clinical records with raw SQL.

| Area | Content |
| --- | --- |
| Laboratory profile | "Demo Toxicology Laboratory" with a fictitious address, phone number, director and CLIA ID |
| Clients | 4 clinics: pain management, opioid treatment program, family medicine and behavioral health. Each has 1–3 locations and codes starting with `DEMO-`. |
| Practitioners | 8 doctors and nurse practitioners spread across the clinics |
| Delivery destinations | Fax, email, mail, courier and pickup |
| Patients | 12 patients. Fixed birth dates between 1950 and 2005. MRNs starting `DEMO-`, assigned by the demo clinic. No SSNs. Deliberate edge cases: two patients with the same name and birth date at different clinics (identity hold), a hyphenated surname, an apostrophe, an accented letter, and a patient with no given name. |
| Demo limits | Values for the catalog's blank cutoffs, applied to the drafts and labeled "demo value, not a recommended cutoff" in the report, so demo results classify |

The shipped demo dataset contains no staff, analyzers, lots, orders or results.

### Fictitious identifier rules

| Data | Rule | Source |
| --- | --- | --- |
| Phone and fax | Real area code with a line number from 555-0100 to 555-0199, reserved for fictitious use. Other 555 numbers are not safe. | DOM-108 |
| Email and web addresses | Under `example.com`, `.example` or `.test` | DOM-109 |
| NPI | Valid check digit with the `80840` prefix (`lis-identity` refuses others). No reserved test range was found. Each one was checked as absent from the NPPES registry when the file was written, and the check is recorded in the demo README. | DOM-103, DOM-132 |
| CLIA ID | Format: two digits, `D`, seven digits. Use `00D0000000`. State code 00 is assumed unassigned; that assumption remains unverified in the demo content notes. | DOM-114 |
| SSN | None | — |
| Patient names | Realistic given names with coined surnames, for example "Marisol Fennimoor" or "Darnell Quillbrook". Every surname was checked against the US Census 2010 surname list when the file was written; the check is recorded in `crates/lis-engine/demo/README.md`. | DOM-110, DOM-131 |
| Street addresses | Fictitious streets such as "Example Way" in a real state and ZIP code (assumption) | — |

## Engine and UI

**Files.** Both files are versioned JSON embedded in the build:

- `crates/lis-catalog/standard/catalog-v1.json`;
- `crates/lis-engine/demo/demo-v1.json`.

Tests check that:

- every reference resolves;
- every entry and value names a source;
- every LOINC code and display name match the pinned release;
- every demo identifier follows the [rules](#fictitious-identifier-rules).

**Commands and queries:**

| Name | Kind and capability | Does |
| --- | --- | --- |
| `catalog.preview_standard_catalog` | Query, `manage_configuration` | Both trees, with each item's state: new, already present, or code in use |
| `catalog.load_standard_catalog` | Command, `manage_configuration` | Creates the chosen items and the items they need, using the existing create and draft functions in one transaction. One summary audit event holds the file version and digest. Running it again is safe. |
| `catalog.generate_catalog_preview` | Command, `manage_configuration` or `approve_configuration` | Renders the report, stores the artifact and records the report |
| `catalog.approve_reviewed_catalog` | Command, `approve_configuration`, with re-authentication | Approve reviewed catalog, as described above |
| `demo.load_demo_data` | Command, demo installation only | Loads the demo file through the identity and reporting commands |
| `demo.complete_setup` (`complete_demo_setup`) | Command, authenticated setup owner in a demo installation only | Completes the persisted first-run choice by loading the whole Standard Catalog and demo file in one transaction, then records completion; requires configuration and order-entry authority |

**UI.** The app has no native menu bar; navigation is the side navigation of the [UI information architecture](../architecture/ui-information-architecture.md).

| Screen | Responsibility |
| --- | --- |
| Laboratory setup › Starter data | Load Standard Catalog and Load Demo Data (demo laboratory only). Links to Catalog review. |
| Laboratory setup › Catalog review (`/configuration/catalog-review`) | Generate Catalog Preview; find, view and save stored reports; Approve reviewed catalog. Works without a starter-data load. |

The starter-content banner and the successful-load screen link to Catalog review. The [UI information architecture](../architecture/ui-information-architecture.md#test-catalog-and-laboratory-setup) identifies the screen and record home.

## Licensing and terminology

| Content | Ship? | Condition | Source |
| --- | --- | --- | --- |
| LOINC codes and display names | Yes | Include Regenstrief's notice and license with every copy, and on the download page. Keep each code with its LOINC display name. | DOM-017, DOM-106 |
| CPT codes and descriptors, CPT drug class list | No | Excluded from the bundle. Any future licensed-code import requires a scoped implementation and rights review (Q-16/Q-21). | DOM-105 |
| HCPCS Level II | Not bundled | Future code-content distribution requires rights and scope review. | DOM-107 |
| Federal workplace cutoffs | Not a clinical starter default | Preserve source/program applicability if evidence is discussed; workplace values do not establish clinical settings | [Content notes](../../crates/lis-catalog/standard/content-notes.md) |
| UCUM unit strings such as `ng/mL` | Yes, as plain text | Check whether the UCUM license applies to using unit strings, not only to redistributing the UCUM tables | DOM-019 |
| SNOMED CT | Not used | Needs a license | ENG-39 |

## Never shipped

| Content | Why |
| --- | --- |
| Invented cutoffs or limits in the Standard Catalog | [AGENTS.md](../../AGENTS.md) domain invariants, Q-03 |
| Validity thresholds and actions | Laboratory policy (Q-24) |
| Interpretive rules, medication-to-analyte mappings, "consistent or inconsistent" logic | Laboratory content subject to the intended-use assessment (Q-22, Q-26) |
| Payer coverage rules, panels sized to billing tiers, preselected diagnoses or tests | INV-13, DOM-032, DOM-038, DOM-039, SR-049 |
| Vendor analyzer interface specifications | Often licensed manuals ([AGENTS.md](../../AGENTS.md#data-and-external-actions)) |

## Requirements and sources

Acceptance criteria are REQ-OPS-014–016 in the [functional requirements](../requirements/functional-requirements.md). Sources are registered in the [domain source register](../research/sources-domain.md#starter-catalog-and-demo-data-sources-2026-10-13).
