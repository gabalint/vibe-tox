# Demo data file (`demo-v1.json`)

Fictitious clients, practitioners and patients for the demo laboratory ([starter data brief](../../../docs/planning/starter-data-brief.md#demo-data)). Everything in the file is invented. It holds no staff accounts and no SSNs. Its demo cutoffs are synthetic values for cutoffs the Standard Catalog leaves blank, each labeled "Demo value, not a recommended cutoff". The loader (`demo.load_demo_data`, `crates/lis-engine/src/demo/`) creates every record through the engine commands, never with SQL, and only in a demo installation.

## Format

Strict JSON, UTF-8, camelCase keys. Records map one-to-one onto the input types named below. Fields marked *loader* are read by the loader only and are not part of any engine input.

| Path | Type | Maps to |
| --- | --- | --- |
| `format` | `"vibe-tox/demo-data"` | Loader refuses any other value |
| `formatVersion` | `1` | Loader refuses unknown versions |
| `dataVersion` | `"1.1.0"` | Recorded in the load audit event |
| `notice` | text | Shown at load |
| `laboratoryProfile` | object | `lis_identity::SetLaboratoryProfile`: `name`, `addressLines` (1-3), `cliaId`, `director`, `phone`, `timeZone`. `expectedVersion` is `null` (the loader supplies it); `billingNpi`, `taxId`, `reason` are omitted (all optional) |
| `clients[]` | object | `CreateClient`: `code`, `name` |
| `clients[].locations[]` | object | `AddClientLocation`: `code` (unique within the client), `name`; the loader supplies `clientId` |
| `clients[].destinations[]` | object | `CreateDeliveryDestinationInput`: `channel` (`fax`, `email`, `mail`, `courier`, `pickup`), `address` (omitted for pickup; optional for courier), `label`; the loader supplies `clientId`. *Loader:* `locationCode` (optional) names a location of the same client and resolves to `clientLocationId`; absent means client-level |
| `practitioners[]` | object | `RegisterPractitioner`: `npi`, `familyName`, `givenNames` |
| `demoCutoffs[]` | object | *Loader only.* `method` and `analyte` (Standard Catalog codes), `value` (lexical decimal string, in the method analyte's unit) and `label`, which must be exactly `Demo value, not a recommended cutoff`. Fills that cutoff in the loading user's draft of the method when it is blank; never replaces a value. Optional; absent means none |
| `patients[]` | object | `RegisterSubject`: `demographics` (`DemographicsInput`: `familyName`, `givenNames` optional, `birthDate` as `YYYY-MM-DD`, `administrativeSex` optional), `source`, `identifiers[]` (`IdentifierInput`: `assigningAuthority`, `identifierType`, `value`). *Loader:* `key` (file-local id, `P01`...) and `acknowledgeCandidates` (keys of earlier patients), which the loader resolves to `acknowledgedCandidates` subject IDs |

Conventions:

- **Codes.** Client and location codes start with `DEMO-` and use only letters, digits, `.`, `_` and `-` (the catalog `code()` rule, 64 characters at most). Identity commands only trim and bound the text, so this is stricter than they require. Client names end in ` (Demo)`.
- **MRNs.** The assigning authority is the client code plus `-MRN` (for example `DEMO-LARKENFIELD-MRN`), the identifier type is `MR`, and the value starts with `DEMO-`. Each clinic therefore has its own MRN namespace.
- **Administrative sex.** Free text up to 64 characters in the engine, never mapped. The file uses `F`, `M` and `U`, and omits it for one patient.
- **Source.** `demo-data` for every patient.

## Content

| Record | Count |
| --- | --- |
| Laboratory profile | 1 ("Demo Toxicology Laboratory", CLIA `00D0000000`, `America/Chicago`) |
| Clients | 4 (pain management, opioid treatment program, family medicine, behavioral health) |
| Client locations | 8 (1-3 per client) |
| Delivery destinations | 15 (fax 6, email 4, courier 2, pickup 2, mail 1; 8 attached to a location) |
| Practitioners | 8 |
| Patients | 12 |
| Demo cutoffs | 5 |

Patient edge cases: `P07` and `P08` share name and birth date at different clinics (Larkenfield and Kestrelby), so registering `P08` opens a possible-duplicate identity hold and needs `acknowledgeCandidates: ["P07"]`; `P04` has a hyphenated surname; `P05` an apostrophe; `P06` accented letters; `P09` has no given name and no administrative sex.

The laboratory profile needs no billing NPI or tax ID, so none is given.

## Demo cutoffs

Aligned with `crates/lis-catalog/standard/catalog-v1.json` content version `1.1.0`. One entry for every drug-analyte cutoff that file leaves blank, and none for a cutoff it fills. A blank means the catalog does not supply a sufficiently matched published value, not that no laboratory publishes one. The values follow one plainly synthetic rule, so none can pass for a recommendation:

| Method | Value |
| --- | --- |
| Urine presumptive screen (`UR-IA-SCREEN`) | `100` |
| Oral fluid presumptive screen (`OF-IA-SCREEN`) | `10` |
| Urine definitive methods (`UR-LCMSMS-*`) | `50` |
| Oral fluid definitive methods (`OF-LCMSMS-*`) | `5` |

The urine specimen validity method (`UR-VALIDITY`) gets none: validity criteria are laboratory policy (Q-24), never shipped. Demo numbers are invented for exercising the application and are never put in the Standard Catalog file. The Catalog Preview labels values matching the current demo file "Demo value". A unit test checks that every entry names a blank cutoff of the shipped catalog and that every blank drug cutoff has one; re-check when the catalog content version changes.

The five synthetic entries are urine amobarbital (`50`), oral-fluid norfentanyl (`5`), 7-aminoclonazepam (`5`), MDA (`5`) and the MDMA screen (`10`), all in `ng/mL`.

Existing installations retain previously saved values: updating or reloading either file does not overwrite them. Preview source labels compare saved numbers with the files in the current build; an earlier demo number can therefore appear as a difference from the current starter value, or as a match to a published value. These comparisons do not reconstruct the origin of an earlier entry. Review the saved method draft to adopt new values; approved method versions need a new draft and approval.

## Loader

- **Where.** Only in a demo installation (`ops_installation.kind = 'demo'`); a laboratory installation refuses it with a `conflict`.
- **Who.** The signed-in user needs `manage_configuration` (Administrator) for the profile, clients, locations, destinations, practitioners and method drafts, and `enter_orders` (Accessioner) for the patients. Each domain command checks its own capability.
- **Once.** The parties load only when none of the file's client codes exists, in one transaction with one `demo.load_demo_data` audit event (file digest and counts). When all exist, loading again creates nothing; when only some exist, it is refused. The laboratory profile is set only when none exists.
- **Demo cutoffs.** Filled through `update_method_draft` in the same transaction. Only a draft's author may edit it, so the Standard Catalog must be loaded first, by the same user; drafts of another author are counted as not filled. Loading again fills cutoffs that are still blank.
- **Keys and codes.** `locationCode` resolves within its client; `acknowledgeCandidates` resolves to the subjects of earlier patients, which `RegisterSubject` must acknowledge, so `P08` opens a possible-duplicate identity hold naming `P07`.

## Synthetic-data provenance and maintenance

The file must remain fictitious and isolated from laboratory installations.
Current content uses the reserved telephone/example-domain conventions in the
sources below. NPI check digits and syntactic validity do not establish that a
number is reserved for demos.

The source check on **2026-10-13** found no NPPES record for each of the eight
synthetic practitioner identifiers listed below. Patient/practitioner surnames
were checked against the Census 2010 list, including hyphenated parts,
accent-folded forms and apostrophe suffixes. These are dated provenance checks,
not continuing guarantees that a name or number will remain unused. Recheck when
fixture content changes. Preserve checks and known limits with the data; routine
script logs belong in local review history.

Census file: `https://www2.census.gov/topics/genealogy/2010surnames/names.zip` (public-domain US Census Bureau data; 162,254 surnames), downloaded 2026-10-13.

- zip sha256: `117c41cb4668727b7627b2845b6df3f83eb2a22a1813f42c0ff4bdcab86de135`
- `Names_2010Census.csv` sha256: `6b4933178060b4e5ac8dd9a611ec15472d35eab98fe9027474bc5ccc5b7a8708`

| NPI | Practitioner | NPPES result, 2026-10-13 |
| --- | --- | --- |
| 2395716998 | Anika Tessingham | No result |
| 1391039819 | Theodore Marrowby | No result |
| 1923772028 | Rosalind Dunmarrow | No result |
| 1299566574 | Emeka Harrowmere | No result |
| 2767634027 | Lucia Corbinwyck | No result |
| 2319071785 | Samuel Sundermere | No result |
| 2280606353 | Hana Eldermoor | No result |
| 1307140347 | Gideon Bexhallow | No result |

Not verified:

- That CLIA state code `00` is unassigned. CLIA numbers begin with a two-digit state code; `00` is assumed unused, and no CMS list was found to confirm it.
- That the coined organization names are not in use. Only people's surnames were checked.
- NPPES lists only registered providers. Absence today does not stop CMS from issuing the same number later. Re-run the check when the file changes.

## Sources

| Topic | Publisher and URL | Checked |
| --- | --- | --- |
| Fictitious telephone numbers: 555-0100 to 555-0199 | NANPA, <https://www.nanpa.com/numbering/555-line-numbers> | 2026-10-13 |
| Reserved example domains (`.example`, `.test`, `example.com`) | IETF, RFC 2606, <https://www.rfc-editor.org/rfc/rfc2606> | 2026-10-13 |
| NPI registry lookup (DOM-132) | CMS, NPPES NPI Registry API, <https://npiregistry.cms.hhs.gov/api-page> | 2026-10-13 |
| Surname list (DOM-131) | US Census Bureau, Frequently Occurring Surnames from the 2010 Census, <https://www.census.gov/topics/population/genealogy/data/2010_surnames.html>, file <https://www2.census.gov/topics/genealogy/2010surnames/names.zip> | 2026-10-13 |
| CLIA number format | CMS, Clinical Laboratory Improvement Amendments, <https://www.cms.gov/medicare/quality/clinical-laboratory-improvement-amendments> | 2026-10-13 (page reachable; the format is also checked by `lis-identity`) |
