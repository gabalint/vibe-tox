# Result-file intake contract

Status: **implemented** for approved long and wide delimited exports, manual loading and watched folders. This contract covers REQ-INT-005/009, REQ-ANA-002/003 and REQ-RES-001. [Operator instructions](../operations/result-files.md) explain setup and recovery; [LC-MS/MS export research](../research/lcms-result-exports.md) retains vendor-specific evidence and source qualifications. Actual laboratory exports require template acceptance and side-by-side validation.

## 1. Workflow and supported scope

The vendor quantitation/review software exports a reviewed batch. Patient sample names must match LIS container labels; controls and calibrators use their configured lot labels. Manual and folder intake preserve the exact bytes as durable receipts before importing them. Approved source registrations bind layout and analyte mappings to a version. Upstream review never substitutes for LIS technical review.

Current supported shapes are one row per sample/analyte (**long**) and one row per sample with analyte columns/groups (**wide**, including two header rows). TargetLynx blocks, Shimadzu sections, header-less positional LIMS files, XML and Excel remain unsupported proposals. No parser infers a vendor's default format.

## 2. Shared safety boundary

The [order-to-result contract](../architecture/order-to-result-contract.md#receipt-and-import) owns patient/QC separation, exact value provenance, duplicate handling, reprocessing and review. File intake adds exact template/header validation and explicit treatment of retained unordered measurements. The laboratory owns the export settings, clinical word mappings, method limits and upstream review procedure.

## 3. Current decisions

The F identifiers remain stable for requirement and source references. They describe the implemented boundary except where explicitly marked unsupported.

| ID | Current contract |
| --- | --- |
| F1 | A file uses the ordinary durable receipt/artifact/outbox route. Record origin, filename, byte length, modification time, digest and actor. Identical bytes are duplicate receipts. |
| F2 | A file template belongs to a versioned result-source registration with declared software/version, analyte mappings and final-concentration policy. An administrator approves under ADR 0015/0018; permitted self-approval requires confirmation. Import attempts retain the registration/parser version. |
| F3 | Match the approved one- or two-row header exactly within the first 50 physical lines. Missing, renamed, reordered or added columns produce a whole-file `layout_mismatch`; preserve the bytes. |
| F4 | Long and wide layouts are implemented. Blocks per analyte remain unsupported until separately scoped and qualified for a laboratory. |
| F5 | Map columns by position within the exact header. Required: sample label, analyte and value (analyte/value groups for wide files). Optional: units, sample type, flags, operator, acquisition text and dilution. Retain source-cell location; unmapped columns remain in the original file. |
| F6 | UTF-8 with optional BOM, or explicitly selected Windows-1252; comma/tab/semicolon delimiters; quoted fields; decimal point only. Trimming is a template choice. Invalid text/UTF-16 fails explicitly. |
| F7 | Map exact nonempty value words only to below LOQ, below LOD or not performed. Retain the original word and approved rule reference. Bounds come from the frozen method, never a guessed value; empty cells cannot be mapped. Zero/negative values need an exact permitted word rule. Positive numeric/censored values cannot be redefined as words. A word whose method limit is missing is quarantined, and a word on a dilution execution is refused because the undiluted limit does not apply. |
| F8 | Use either the exact units column or a declared template unit. Declared units are retained in measurement text and attributable to the registration. Mapped-word units come from the frozen method. No unit conversion or inferred alias. |
| F9 | Sample-type words cross-check LIS position roles. A disagreement is quarantined. Unplaced calibrator/blank rows may be set aside only when their label matches neither a known container nor lot; other unmatched material remains quarantined. |
| F10 | Match analyte and sample labels exactly, subject to configured trimming. Explicit **not a result** names count without creating results; unknown analytes remain quarantined. |
| F11 | Source final-concentration declaration and configured dilution column must agree with execution context. The LIS never multiplies a result by a dilution factor. |
| F12 | Identical rows add no evidence; changed rows require attributable conflict/reprocessing review. After a template fix, **Retry import** the retained receipt; loading identical bytes again cannot silently reinterpret them. |
| F13 | Poll an enabled watched folder at startup and every 10 seconds while the app is open, including at sign-in. Keep source files in place. Folder/current-source validation is rechecked when recording the receipt. |
| F14 | Intake limit 20 MiB; parser limits 50,000 rows, 512 columns and 4 KiB per field. Oversize intake records an aborted receipt without reading/storing the file; parser-limit failures retain received bytes and an explicit failed attempt. |
| F15 | Dependencies follow the library policy. The implemented reader uses the approved `csv` crate and bounded Windows-1252 decoding; filesystem polling needs no watcher dependency. |
| F17 | File-source analytes outside accepted order scope remain usable **measured, not ordered** evidence, with no result line or queue item. An accepted add-on and a reviewer action with reason can use them; technical review and all release gates still apply. ASTM's Not ordered queue remains distinct (Q-41). |

## 4. Template and value handling

A template stores exact headers, shape, column positions, encoding, delimiter, trimming/end-of-data behavior, sample-type/value-word mappings, excluded analyte names and an optional declared unit. Wide analyte names must match their header cell/group label. The UI sample/check stage stays in memory and does not create a clinical receipt; the saved template contains the layout rather than the patient's sample file.

The check preview uses the same strict value reader as import and applies the F7 value rules. It reports mappings and value forms; actual run placement and clinical eligibility are established only during import. The current preview's quantitative reading of values is an explicit limitation for qualitative analytes.

## 5. Receipt and row outcomes

A receipt is durable before parsing. Every attempt accounts for imported, duplicate, quarantined, not-result, set-aside and retained rows. Controls/calibrators/blanks use QC evidence; patient evidence uses measurements/results. Whole-file failures retain the receipt and reason. A row's source location and original lexical value remain available after review.

An unplaced nonpatient batch row is visible as **batch row not placed**. Unusable outside-order rows retain reasons such as incompatible unit, malformed value, dilution mismatch or unsupported dilution. Usable unordered measurements can be selected only for an accepted component on the correct order/accession/matrix; the engine rechecks specimen/order gates and expected result revision. It creates result lines only for analytes with retained evidence.

Partial imports do not reconcile absent analytes. **Results expected** follows frozen execution scope, including closed runs; failed executions defer to explicitly linked repeats. Retained receipts provide the exact retry route. A missing placeholder does not count as completed analytical work.

## 6. Manual load and watched folder

| Boundary | Implemented behavior |
| --- | --- |
| Folder selection | Rust native dialog; canonical readable folder unique to one instrument. Local folders or mapped laboratory shares are supported subject to host qualification. |
| Existing content | Baseline files present when the folder is selected are recorded and not automatically imported; manual loading remains possible. |
| Candidate files | Direct regular `.csv`/`.txt` files only; skip subfolders, links, hidden files and temporary-file patterns. |
| Stability | Size/time stable over two scans at least 10 seconds apart; Windows also refuses an exporter still holding a writer handle. Check size again after reading. |
| Deduplication | Ledger uses instrument, name, size and modification time; content digest identifies duplicate bytes. Same-name/same-size/same-time overwrites can be missed, so each export needs a new filename. |
| Enablement | Instrument enabled state controls folder pickup; an instrument without a folder supports manual loading only. |
| Health | Missing/unreadable folder is an intake alert; waiting and oversized files remain counted warnings. |
| Source files | Never moved or deleted by intake. |

## 7. User-facing guidance

Pages, fields, actions, gates and recovery steps are in [result-file operation](../operations/result-files.md) and the built-in help. Source/template changes require revalidation, including a batch compared with the vendor application. Template approval follows F2.

## 8. Setting up the export in the vendor software

Software/version-specific evidence is in the [vendor export routes](../research/lcms-result-exports.md#vendor-export-routes); the laboratory workflow (fixed layout, final concentrations, precision, units/dilution, unique filenames, export after batch review, no spreadsheet editing) is the [operator procedure](../operations/result-files.md#1-set-up-the-export-in-the-vendor-software).

Header checks do not detect changed units, precision or calculation settings that leave headers identical. Treat those changes as configuration changes requiring comparison and template acceptance. Public vendor evidence does not qualify a partner's installed version or export.

## 9. Implementation owners

| Owner | Contract |
| --- | --- |
| `lis-analytical` | Versioned file layout, validation and source approval; accepted execution/component association. |
| `lis-interfaces` | Durable file receipts, origin, baseline/seen-file ledger, size limit and folder identity. |
| `lis-results` | Pure delimited reader, mapped-word semantics, row outcomes, quarantine/reprocessing and use of retained measurements. |
| `lis-engine` / `src-tauri` | Intake workers, capability/authorization registration, audited orchestration and native dialog preflight. |
| UI | Uses generated types and engine gates; does not duplicate parsing or clinical decisions. |

Exact structs, migrations and command signatures live with their owning source; changes require affected-caller and generated-binding review. Shared migration rules belong in the developer/store contract, not this feature document.

## 10. Verification and remaining limits

Test cases must cover exact headers/preambles, quoted delimiters, encoding/field limits, long/two-row-wide layouts, role/unit/dilution conflicts, empty and mapped words, duplicate/reprocessed files, partial output, retained unordered measurements, stale folders, baseline files, interrupted intake and authorization/audit. Preserve real parser/import/storage/gate behavior in these tests.

Actual export fixtures, analyzer software/version behavior, folder-share semantics, upstream approval/certification mapping and laboratory policy remain Q-04/Q-25/Q-40/Q-41. No clinical validation follows from passing synthetic parser cases.
