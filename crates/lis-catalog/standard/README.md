# Standard Catalog file format, version 1

This folder holds the Standard Catalog that ships with the app ([starter data contract](../../../docs/planning/starter-data-brief.md)). `catalog-v1.json` is embedded in the build by `lis-catalog` and loaded only as **drafts** through the existing catalog commands. It is synthetic-safe reference content: it holds no patient data.

The format is the contract between the content author and the loader. The current content version is `1.1.0`; [content notes](content-notes.md) retain its menu/LOINC provenance, clinical cutoff comparisons and laboratory review limits. Content changes require the applicable source review and validation; format changes require coordinator review.

## Top level

| Field | Type | Rule |
| --- | --- | --- |
| `format` | string | Always `"vibe-tox/standard-catalog"` |
| `formatVersion` | integer | Always `1` |
| `catalogVersion` | string | Content version, for example `"1.1.0"`. A new content version never changes what a laboratory already loaded. |
| `reviewStatus` | `"unreviewed"` or `"reviewed"` | `"reviewed"` only after a named clinical reviewer has reviewed this content version (Q-02) |
| `disclaimer` | string | Shown at loading and on the Catalog Preview |
| `loinc` | object | `version` (for example `"2.83"`) and `notice`: Regenstrief's required notice, verbatim |
| `sources` | array of Source | Every source cited anywhere in the file |
| `drugClasses` | array of DrugClass | The project's own pharmacological grouping, not copied from CPT |
| `specimenTypes` | array of SpecimenType | |
| `analytes` | array of Analyte | |
| `methods` | array of Method | |
| `orderables` | array of Orderable | Tests and panels |

Unknown fields are refused (`deny_unknown_fields`).

## Records

**Source:** `id` (unique, used in `sources` lists), `title`, `publisher`, `url`, `published` (date or `null`), `checked` (date), `note` (scope, limits). Each source is also registered in [the domain source register](../../../docs/research/sources-domain.md).

**DrugClass:** `code`, `name`.

**SpecimenType:** `code`, `name`, `sources`.

**Analyte:**
- `code`, `name`;
- `drugClass` (a DrugClass code, or `null` for validity analytes);
- `loinc` (`null` or `{ "code", "displayName" }`, the display name exactly as in the pinned LOINC release);
- `sources`.

Analytes are specific to one specimen type, because a LOINC code names an observation in one specimen by one method. Use the specimen type code as the prefix: `UR-MORPH`, `OF-MORPH`. Class-level screening analytes are distinct from molecules: `UR-OPI-IA` is not `UR-MORPH`.

**Method:**
- `code`, `name`;
- `specimenType`;
- `certainty` (`"presumptive"` or `"definitive"`);
- `outputScale` (`"qualitative"` or `"quantitative"`);
- `thresholdRule` (`"at_or_above"` or `"above"`);
- `thresholdRounding` (`"before_rounding"` or `"after_rounding"`);
- `analytes` (array of MethodAnalyte, at least one);
- `sources`.

`thresholdRule` and `thresholdRounding` are starter values the laboratory must confirm.

**MethodAnalyte:**
- `analyte` (code);
- `unit` (UCUM text, at most 64 characters, or `null`);
- `cutoff` (`null`, or `{ "value", "source", "label" }`).

The cutoff fields:
- `value` is a lexical decimal string such as `"300"` or `"0.5"`;
- `source` is a Source id;
- `label` is the short text the report shows, for example "Published clinical LC-MS/MS cutoff (source laboratory); verify preparation".

Limits of detection and quantitation, reportable ranges and display scale are **not** part of the format: they belong to each laboratory's validated assay and stay blank.

**Orderable:**
- `code`, `name`;
- `kind` (`"test"` or `"panel"`);
- `components` (array of Component);
- `sources`.

A test has exactly one component; a panel has at least one.

**Component:**
- `method` (code);
- `purpose` (`"analytical"` or `"validity"`);
- `analytes` (codes, all measured by that method, at least one, no repeats).

## Rules the loader's tests enforce

1. **Codes.** At most 64 characters: ASCII letters, digits, `.`, `_` or `-`, starting with a letter or digit. Codes are unique within each kind.
2. **Names.** 1–200 characters, no control characters.
3. **References resolve.** Every reference resolves to a record in the file, and every listed source exists.
4. **Specimen types match.** A method's analytes use the method's specimen type prefix.
5. **No invented values.** Every `cutoff` is a non-negative decimal with a label and a `source` that resolves to a listed source. The tests cannot read the publication, so they pin the exact set of filled cutoffs, as (method code, analyte code, value, source id), in [`validate.rs`](../src/standard/validate.rs) (`PINNED_CUTOFFS`). The current pin was transcribed for content version 1.1.0 against the clinical sources and comparisons in [content notes](content-notes.md), matching analyte, specimen and technique. This evidence review is not laboratory approval or clinical validation. Any change to a cutoff fails the test until the pin is updated deliberately, with the new value checked against its publication. A structural test proves transcription consistency, not the source's clinical applicability.

## Rules checked when the content is written

The build cannot check these offline. The content author checks them before each content version is released, and records the check in `content-notes.md`:

1. **LOINC is pinned.** Every LOINC code and display name match the release named in `loinc.version`, as returned by a public LOINC terminology service.
2. **No billing logic.** Panels follow clinical drug classes, never billing tiers. No CPT codes, descriptors or CPT drug class lists appear anywhere.
3. **Sources cover cutoffs.** Every method and orderable lists the source of each cutoff its analytes carry.
