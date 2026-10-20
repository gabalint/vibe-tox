# LC-MS/MS result exports: file formats, prior art and loader engineering

Evidence for the [result files design brief](../planning/result-files-design-brief.md). It asks four questions:

- What files does LC-MS/MS quantitation and batch-review software write for an LIS?
- How do users configure those files?
- How do other systems load them?
- What does a safe loader need?

All sources were checked on 2026-10-13 and are registered as DOM-133 to DOM-141 and ENG-217 to ENG-221.

**How to read the evidence.** Vendor manuals for the export layouts are mostly behind a login or blocked. The shapes below therefore come from public parsers, test files, forum threads and knowledge-base pages. They describe files real users produced, not vendor specifications. Every product lets the user change the columns, so the layout a laboratory actually uses has to be read from its own file. "Verified" means the fact was read in the cited source; "not confirmed" means it was only inferred or seen in a search snippet.

## Vendor export routes

### Agilent MassHunter Quantitative Analysis (B.07 to 12)

**How users export.**

- **Batch table.** Right-click a column header and choose "Export Table…" to write CSV (verified, DOM-133, DOM-134).
  - The exported columns are those the batch table shows. Users add Dil., Amt., Tot. Amt., Multiplier and S/N through Add/Remove Columns.
  - Agilent's forum answer warns that the table export "may not display the complete saved analysed result set" and points to report generation for final results (DOM-134).
- **Report method.** The LIMS report templates `Gen_LimsIntegration.report.xml` (CSV) and `GenLIMs_xmlOutput.report.xml` (XML) exist, with a shipped post-process script that copies the CSV after the report is generated, batch-wise or per sample (verified that these exist, DOM-134).
  - Their column list is not public.
- **Automation.**
  - A sequence can name a data-analysis method and a report template, so processing and reporting can run after acquisition. This was verified for GC/MS B.07.05; LC/MS 10.x to 12 is not confirmed.
  - No source showed an export tied to review or approval. The Compliance edition has a batch-reviewer role (DOM-134).

**File shape (batch table).** Verified in public example files and two independent parsers (DOM-133):

- comma-delimited, ASCII, no BOM, and **wide**: one row per injection;
- **two header rows**:
  - Row 1 holds group labels, written only in the first column of each group: `Sample`, `<compound> Method`, `<compound> Results`, `Qualifier (<m/z> -> <m/z>) Results` and `<ISTD> (ISTD) Results`.
  - Row 2 holds the field names. Data starts on row 3.
- The first two columns have empty headers and hold `!` warning markers.

| Concept | Header text seen |
| --- | --- |
| Sample | `Name`, `Data File`, `Type`, `Level`, `Pos.`, `Vol.`, `Dil.`, `Acq. Date-Time` |
| Per compound | `Exp. Conc.`, `RT`, `Area` (older: `Resp.`), `Height`, `S/N`, `Calc. Conc.`, `Final Conc.`, `Accuracy`, `MI` (manual integration, `TRUE`/`FALSE` in varying case) |
| Qualifier group | `Ratio` |
| Type values | `Sample`, `Cal`, `QC`, `Blank`, `DoubleBlank`, `MatrixBlank`, `ResponseCheck` |

- There is no units column; units are a method property.
- `Final Conc.` = `Calc. Conc.` × multiplier, where the multiplier includes the dilution (DOM-134, Agilent knowledge base).
- **Not-found values are empty cells.** Some third-party parsers turn them into 0, which is a hazard.
- **Dates follow the workstation locale.** Formats seen: `11/25/2015 1:10 PM`, `26/11/2024 3:05 pm` and a probable Taiwanese (ROC) calendar year (`112/4/20`).

### Thermo TraceFinder (3.x to 5.x)

- **Grid export.** `File > Export data to > CSV or Excel` (verified, TraceFinder 5.1, DOM-135). Options cover:
  - format;
  - one worksheet per analyte;
  - exporting only the selected columns and rows.
- **Reports.** Reports can be copied to an export folder. Community posts from 2012 describe LIMS results as XML (`.rsx`) files (DOM-135).
- No official "export to LIMS" step tied to approval was found.
- **Locale.** TraceFinder 5.2 supports only US-English Windows and Office number settings (verified, release notes, DOM-135).
- **Shape.** The grid export is **long**, one row per sample and compound. This is not confirmed and comes from parser behavior.
  - One public parser reads `Compound`, `Filename`, `Area`, `Response Ratio`, `Sample Type`, `Calculated Amt`, `Theoretical Amt`, `Excluded`, `%Diff` (DOM-135).
  - Sample types include `Cal Std`, `QC Std` and `Unknown`.
  - **Not-found is `N/F`**, which that parser turns into 0.

### Thermo Chromeleon 7

- Output formats include TXT, CSV and XLS, with "text or GAML for LIMS" (DOM-136).
- A spreadsheet-like report template defines what is exported. Export can run from the queue ("export report"), including automatically after each injection (7.2 SR5 release notes, DOM-136).
- Report dates follow Windows regional settings.
- No public vendor source documented the preamble length, encoding or delimiter of a results export.
- No public results-table header was found.

### Shimadzu LabSolutions LCMS and Insight

- **Export routes.**
  - "Export Quantitative Results (ASCII Conversion)" from the Quant Browser, with item and delimiter choices (verified, LCMS 5.97 guide, DOM-137).
  - Batch "ASCII conversion" at the end of each run, per batch or per analysis.
  - Neither is tied to review.
- **Shape: sectioned** and tab-delimited, verified on HPLC 5.54 output.
  - Blocks: `[Header]`, `[File Information]`, `[Sample Information]` and `[Compound Results(Detector A)]`, followed by a table.
  - LCMS output adds `[MC Peak Table]` and others.
  - Sample fields include `Sample Type` (for example `0:Unknown`), `Sample Name`, `Sample ID`, `Dilution Factor` and `Vial#`.
  - Compound fields include `Name`, `R.Time`, `Area`, `Conc.` and `Area Ratio`.
  - There is no units column.
- **Not found is written as `0`.**
- Insight has flags and LIS-linkage marketing; no documented CSV export was found.

### SCIEX, Waters and review platforms

**SCIEX OS (Analytics), DOM-139.**

- **Export routes** (verified in the SCIEX OS and Reporter user guides and knowledge-base articles):
  - **Results table export** to `.txt`, with all or visible columns, and visible rows only.
  - **Metric export.** "Export Results Table – Metric" writes one metric for all samples and components, tab-delimited and wide.
  - **Reporter "Create Report".** Formats are docx, txt, html, csv and pdf. Shipped LIMS templates are "SCIEX OS LIMS Template A" (analytes vertical, so long), "…Displaying Analytes Horizontally" (wide) and one for Analyst-acquired data.
  - **"Transfer Results to LIMS".** Requires a locked results table and a LIMS URL. Its file format is not published, and 3.1 had a bug that exported all columns when only visible ones were chosen.
- **Automation.** Batch mode can generate reports automatically after processing. No automatic text export or LIMS transfer was found.
- **Results-table columns** that the guide marks LIS-supported include:
  - `Sample Name`, `Sample ID`, `Sample Type`, `Operator Name`, `Acquisition Date & Time`, `Dilution Factor`;
  - `Component Name`, `Conc. Units`;
  - `Actual Concentration`, `Calculated Concentration`, `Accuracy`;
  - `Area`, `Height`, `Area Ratio`, `Retention Time`;
  - `Ion Ratio`, `Expected Ion Ratio`;
  - `Modified`, `Integration Type`, `Used`, `Reportable`.

  Reporter tags (`Sample_Name`, `Calculated_Concentration`, `Analyte_Unit`, …) are template fields and are not necessarily the exported header text.
- **Sample types:** Standard, Blank, Double Blank, Quality Control, Unknown and Solvent.
- **Words in numeric columns.**
  - Area, Area Ratio and Ion Ratio show `N/A` when no peak is found.
  - Calculated Concentration can hold `N/A`, `degenerate` (no actual concentrations on the standards), `<0`, `>0`, "not enough points" or an infinity sign.
  - No `<LLOQ` string was found.
- **Number format** is set per column (decimals, significant digits or scientific), with a point or comma decimal mark and no grouping. Exported precision probably follows it (not confirmed).
- **Review.** SCIEX OS 3.4 added "mark as reviewed" and "mark as approved", recorded in the audit trail.
  - Separate permissions cover locking, reviewing, approving, LIMS transfer, and "Allow Export of and Create Report for unlocked Results".
  - The Reporter has no reviewer or approval tag, so the approval state can gate the export but is probably not in the file.
  - Rows with Reportable cleared are left out of reports, exports and LIMS transfers.
- **Known issues.** The 2.2 release notes fixed exported dates that ignored regional settings, and a Sample Name column that could not be moved for LIS exports.

**SCIEX Analyst and MultiQuant, DOM-138.**

- **Results table.** File › Export › Results Table, in "MultiQuant" or "Analyst" format, with visible columns or rows, written to `.txt`.
  - "Fit" lines exist in these formats, so a preamble may precede the table (not confirmed).
  - Analyst exports all or visible columns to `.txt` or `.csv`.
- **LIMS report template.** A Reporter LIMS template saved under `C:\ProgramData\AB SCIEX\MultiQuant\Reporter` produces CSV when the output format is CSV. The template must have no picture tags, and the trigger is manual.
- **Settings and gotchas.**
  - MultiQuant 3.0.3 requires US-English number and date settings.
  - SCIEX warns that Excel can corrupt exported text files, and that component names containing commas are quoted.
- Encoding and delimiter are not published.

**Waters MassLynx with TargetLynx, DOM-140.**

- **Complete Summary export.**
  - "Export › Current, Complete or All Groups Summary" writes the summary table as displayed, tab-delimited. Undisplayed columns are not exported (verified, knowledge base).
  - A real public demo file (MIT licence) is ASCII with CRLF and no BOM. It starts with a preamble: a title, filler lines of tabs only, and a "Printed <date>" line.
  - Each compound then opens with `Compound N:  <name>` and has its own header row. That header has 13 fields, the first empty: `# Name Type Std. Conc RT Area IS Area Response Primary Flags nM %Dev S/N`.
  - The concentration column is headed by its unit (`nM`) in that file and by `Conc.` in another parser's files.
  - A missing peak leaves the cells from RT onward empty. Primary flags (`bb`, `MM`, `MM!`, …) are defined in a knowledge-base article.
- **LIMS export.** "Export › LIMS" writes comma-delimited text with fixed decimals.
  - The 2000-era interfacing guide describes header lines for file, sample list and method paths, then per-sample, per-compound records of positional fields with no column names.
  - The 4.x guides are reported to keep this format with a dataset header (search excerpt only).
- **XML export.** Element and attribute names come from a parser and a search excerpt (`analconc`, `pkflags`, `ionratio`).
- **Unquantifiable values.** A MassLynx 4.1 technical note reports `0.000000` for an unquantifiable Conc, and a later release note reports a blank (both from search excerpts). The behavior depends on the version.
- **Review fields.** No export documents a reviewer or approval field. The LIMS file records who modified a peak and when.
- **Other routes.** The MassLynx LIMS Interface sends HL7 from configured templates ([DOM-021](sources-domain.md)). waters_connect MS Quan release notes mention a configurable CSV LIMS export to a folder, with worklist and sample ID columns. That was not verified beyond search excerpts.

**Review platforms, DOM-141.**

- **A hosted review service.** Processing and assessing steps apply laboratory-written quality rules.
  - It delivers "CSV or XML", with custom outputs defined and provided by the vendor (marketing text).
  - A PDF archive keeps the review and certification audit trail.
  - Its column names, and whether the export carries reviewer or certification fields, are not public.
  - Peer-reviewed abstracts describe automated peak review in a clinical mass spectrometry laboratory and its use on a 113-drug urine panel (ENG-298, ENG-299).
- **Chromsystems CSQUANT.** Evaluates Analyst, MultiQuant and MassLynx results. No export documentation was found.

## File shapes

| Shape | Seen in | Loader need |
| --- | --- | --- |
| Long: one row per sample and analyte | TraceFinder grid, SCIEX OS "LIMS Template A", MultiQuant results table | Header row; analyte column; value column |
| Wide, one header row | SCIEX OS "Displaying Analytes Horizontally", metric export | One column per analyte |
| Wide, two header rows with group labels | MassHunter batch table | Carry group labels forward; key = group + field; groups without the value field are not results |
| Blocks per analyte: a title line, then a table | TargetLynx Complete Summary | Analyte from the title; the same header in every block |
| Per-sample sectioned key/value blocks | Shimadzu ASCII | A block reader; sample fields from the block header (not in the first version) |
| Positional records without a header | Waters LIMS export | Field positions per version (not in the first version) |
| Preamble lines before the header | TargetLynx (title and printed date); possibly MultiQuant fit lines and Chromeleon report-template headers | Header line number or block title |

Text features found:

- comma, tab and semicolon delimiters, and CRLF and LF line endings;
- locale-dependent dates without a time zone;
- display-dependent precision;
- not-found written as empty, `N/F`, `N/A`, `0` or `0.000000`;
- SCIEX's words `degenerate`, `<0`, `>0` and `not enough points`.

Windows "Unicode text" is UTF-16LE with a BOM and tab-delimited. That comes from a Microsoft forum post (ENG-217) and was not confirmed in a vendor file. No SCIEX or Waters export documents a reviewer or approval column.

## Loader engineering

| Topic | Finding | Choice in the brief |
| --- | --- | --- |
| CSV rules | RFC 4180 is informational: CRLF, an optional header, quoted line breaks, doubled quotes (ENG-217) | Read RFC 4180 quoting with flexible field counts, and quarantine ragged rows |
| Excel | Excel's separator follows Windows regional settings. Opening a file can strip leading zeros, turn long numbers into scientific notation and keep only 15 digits (ENG-217) | SOP: never re-save exports in Excel. LIS container labels start with a letter, so zeros are not stripped |
| Change notifications | `ReadDirectoryChangesW` works over SMB 3, but its network buffer is limited to 64 KB and can overflow (ENG-218). inotify does not report changes made by other clients on network file systems. The `notify` crate tells users to poll on network mounts | Poll; no `notify` |
| Complete files | On Windows, an open that refuses sharing fails with a sharing violation while a writer holds the file (ENG-219). Linux and macOS have no mandatory locks. Writing to a temporary name and renaming is the producer-side convention | Stability across two scans, a Windows sharing check, a size re-check after reading, and skipping temporary names |
| After import | Source bytes and import provenance must remain available for reconciliation (REQ-INT-009) | Keep the bytes; leave source files in place in the first build |
| `csv` crate | 1.4.0 (2025-10-17), Unlicense or MIT, already in `Cargo.lock` through Typst (ENG-220). Supports flexible rows, byte records, no-header mode and custom delimiters. **No size limits**; an unterminated quote swallows the rest of the file | Use it with our own byte, row, column and field limits; check the last record |
| Encodings | `encoding_rs` 0.8.42 is not in the lockfile. UTF-16 decodes with `std` (`String::from_utf16`; `from_utf16le` is not stable on the pinned 1.97.1). Windows-1252 differs from Latin-1 only in 0x80–0x9F (ENG-220) | UTF-8 and Windows-1252 (a 32-entry table) with `std`; UTF-16 added when a laboratory's file needs it |
| Hostile input | Link following (CWE-59) and the gap between checking a path and opening it (ENG-221) | Regular files only, links not followed, the path re-checked at open, content only ever treated as text |

## Open points

- The Agilent LIMS CSV template's columns, and any approval-gated automatic export in MassHunter, SCIEX OS or TraceFinder, need the laboratory's installed software to confirm.
- Encodings of real vendor files: the public MassHunter files may have been re-saved. Each laboratory's sample file settles it at template setup.
- Shimadzu LCMS quantitation ASCII with qualifier ratios was not found publicly.
- Some file details are unpublished:
  - SCIEX's export encoding and delimiter;
  - the "Transfer Results to LIMS" file format;
  - the waters_connect MS Quan CSV columns;
  - the contents of the hosted review service's export.

  The laboratory obtains them from its own installation or supplier, under Q-04 and Q-25.
