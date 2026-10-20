# Loading results from LC-MS/MS result files

Before using a file source for patient results, the laboratory must:

- accept its template;
- compare a batch side by side in the vendor software and in the LIS;
- approve its procedure.

Design and reasons: [result files design brief](../planning/result-files-design-brief.md).

The quantitation or batch-review software (MassHunter, SCIEX OS, MultiQuant, TraceFinder, Chromeleon, a review platform) exports the reviewed batch as a CSV or TXT file. The LIS reads that file through an approved **file template**. Every row lands somewhere visible:

- a patient measurement waiting for technical review;
- QC evidence;
- quarantine;
- a kept "measured, not ordered" value;
- a set-aside row.

## 1. Set up the export in the vendor software

- **Sample name.** Each injection's sample name must be the LIS label: the tube's container label, or the lot's sample label for controls and calibrators. Scan the barcode rather than typing it.
- **When to export.** Export after batch review or approval, not after acquisition. In SCIEX OS, give nobody the permission to export unlocked results.
- **Fixed layout.** Save one fixed export layout and protect it. Include:
  - sample name;
  - sample type;
  - analyte (long files) or analyte groups (wide files);
  - the final, dilution-corrected concentration;
  - units, if the product can export them;
  - dilution factor;
  - flags.
- **Numbers.** Use a decimal point, no thousands separators, no scientific notation, and at least the laboratory's reporting precision.
- **"Not found".** Where the product can write a word such as `N/F` for "not found", use it. Empty cells are always quarantined.
- **Excel.** Never open and re-save an export in Excel before loading it.
- **One file per batch.** Name it with the batch and export time.

Per-product notes are in the [vendor export routes](../research/lcms-result-exports.md#vendor-export-routes).

## 2. Set up the instrument

1. Go to **Laboratory setup › Instruments** and register the instrument with the connection type **Result files**.
2. **Optional: choose a watched folder.** Use **Choose folder**. Rules for the folder:
   - Use a dedicated folder, local or on a mapped lab-LAN share, that only this instrument's exports go to. One folder belongs to one instrument.
   - Files already in the folder when you choose it are **not** loaded automatically. The LIS shows how many there are, and you can load any of them by hand.
   - The LIS reads the folder only while the application is open, including at its sign-in screen. Files exported while it is closed are read at the next start.
   - Files are never moved or deleted.
3. **Folder pickup** is switched on and off with the instrument's enable setting. A result-files instrument without a folder is for manual loading only.

## 3. Create and approve the file template

1. Go to **Bench setup › Instruments**, open the instrument and choose **New file template**.
2. Enter the software name and version, and say whether the file reports final (dilution-corrected) concentrations.
3. **Open a sample export**, choose the delimiter (and Windows-1252 if the file is not UTF-8), and click the header line. A MassHunter batch table has a second header row; say so.
4. Choose the shape:
   - **Long:** one row per sample and analyte. Pick the analyte and value columns.
   - **Wide:** one row per sample. Pick the analyte columns, or the value field of each compound group.
5. Map the columns, every analyte name (or mark internal standards **not a result**), the sample-type words, and the value words (§4).
6. **Units:** give a units column or declare one unit.
7. **Check** shows what every row of the sample would become. Then **Save draft**.
8. **Approve.** On the instrument's source, **Check and approve** opens the template. **Check with a file** runs it on a file of the approver's choice, and **Approve source** activates it. Someone other than the author approves, unless the laboratory's [separation-of-duties policy](README.md#separation-of-duties-and-one-person-laboratories) allows self-approval. The approver should compare one batch in the vendor software and in the LIS before approving.

A file whose header differs in any way from the approved template is refused whole. When the export changes, create and approve a new template version.

## 4. Value words (laboratory decision Q-40)

Exports write "nothing found" or "could not calculate" as words: `N/F` (TraceFinder), `N/A`, `degenerate`, `<0`, `>0` (SCIEX). In the template, each word the laboratory approves can mean one of three things:

| Meaning | What the result becomes |
| --- | --- |
| Below LOQ | `<` the method version's limit of quantitation, in the method's unit |
| Below LOD | `<` the method version's limit of detection |
| Not performed | Not performed |

The measurement keeps the word exactly as received. The result names the rule and template version that gave it.

These are always quarantined, whatever the template says:

- **Empty cells.** Vendors also leave a cell empty when a value could not be calculated, for example after an internal-standard failure.
- **Unmapped words.**
- **Zero or negative values**, unless mapped.
- **Words on a dilution run.** The method's limit is not the limit of a diluted injection, so a word on a dilution run never becomes a result.

A word that reads as a positive number cannot be mapped.

## 5. Load a file

- **By hand:** **Review › Result imports › Load results file**. Choose the instrument, then the file. A file already received says so and is not imported again. A file over 20 MiB is not read.
- **Watched folder:** the LIS waits until a file has stopped changing (two checks at least 10 s apart), then reads it. A refused or oversized file shows in instrument health.

The import attempt shows:

- how the file arrived (loaded by whom, or from the folder);
- its name, size and SHA-256;
- the template version;
- what happened to every row.

## 6. What happens to each row

| Row | Result |
| --- | --- |
| Patient sample on an open run, ordered analyte | Measurement and result waiting for LIS technical review |
| Control, calibrator or blank on the run | QC evidence |
| Calibrator or blank not on the run (and not a container or lot label) | Listed as **set aside** (not quarantined) |
| Analyte marked not a result (internal standard) | Counted |
| Patient sample, analyte measured but **not ordered** | Kept as **measured, not ordered** (§7) |
| Unknown label, unknown analyte, wrong unit, sample type that disagrees with the run position, dilution that disagrees with the LIS, wrong field count, empty or unmapped value | Quarantined with the reason |
| Same row exported again | Counted as a duplicate |
| Changed row exported again | Quarantined as a conflict until a reviewer accepts it as reprocessing |

## 7. Measured, not ordered (laboratory decision Q-41)

A batch file carries every analyte of the method. The LIS loads results only for ordered analytes. It keeps the others as measurements, listed on the specimen's results under **Measured, not ordered**, with no queue item.

If the analyte is later added to the order as an accepted add-on, a technical reviewer can choose **Use measured result** on that row and give a reason. The add-on joins the original accession and the result line is created from the kept measurement. It then goes through technical review, QC and every release gate like any other result. Whether to tell the ordering provider about an unordered positive finding is laboratory policy.

## 8. When a file is refused

| Message | What to do |
| --- | --- |
| The columns differ from the approved template | Fix the export in the vendor software, or approve a new template version, then **Retry import** on the same attempt |
| The text cannot be read | Save the export as UTF-8, or approve a template version that reads Windows-1252, then **Retry import** |
| Too many rows, columns or characters | Export the batch in smaller files |
| No file template | Approve a file template for the instrument, then **Retry import** |
| Too large (over 20 MiB) | Export the batch in smaller files |

## 9. Limitations

- **Supported shapes:** CSV and TXT exports in long or wide shape. Waters TargetLynx summary blocks, Shimadzu ASCII sections, Waters' header-less LIMS export, XML and Excel files are not supported yet.
- **Not detectable:** a change of units, precision or calculation settings in the vendor software does not change the header. Treat any such change as a configuration change: a comparison batch, then a new template version.
- **Same-size copy:** a re-export copied over an old file with the same name, size and time is not noticed. Give every export a new file name.
- **Template check:** it reads values as quantitative even for a qualitative analyte.

## Partial output and retained reconciliation

A successful import does not mean every ordered analyte arrived. **Results expected** compares each active patient execution with its frozen analyte scope, including closed runs. Partial imports leave missing analytes in that queue across restart. Retry uses the retained receipt and links to its exact latest import attempt; it never requires guessing an ID. An appropriate terminal disposition can reconcile an analyte; a missing-value placeholder does not. Failed earlier executions give way to their explicitly linked repeat, whose full ordered scope must itself be reconciled.
