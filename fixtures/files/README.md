# Synthetic LC-MS/MS result files

Synthetic data only. Every sample label, lot label, batch name, file name, value, unit and flag in these files was made up for tests. They describe no real patient, laboratory, instrument export or assay, and the numbers are not cutoffs, limits or reference values. The shapes imitate the public descriptions in the [LC-MS/MS result exports research](../../docs/research/lcms-result-exports.md); they are not vendor files.

Container labels use the LIS format (`C00000123`); controls and calibrators use lot sample labels (`QC-LOW-L1234`, `CAL-1`).

The exact bytes matter: line endings, byte-order marks and encodings are part of each case. `.gitattributes` keeps git from converting them; do not re-save these files in an editor or spreadsheet.

| File | Imitates | Text | Case |
| --- | --- | --- | --- |
| `sciex_long.csv` | SCIEX OS LIMS template, long | UTF-8, CRLF, comma | `N/A`, `<0`, `>0` and `degenerate` in the concentration column; an internal standard row; a units and a dilution column |
| `tracefinder_long.csv` | TraceFinder grid export, long | UTF-8, LF, comma | `N/F` values; a `Sample Type` column (`Unknown`, `QC Std`, `Cal Std`); the label in `Sample ID` |
| `masshunter_wide.csv` | MassHunter batch table, wide | UTF-8, CRLF, comma | Two-row header with group labels; two empty `!` marker columns; repeated field names; a qualifier group and an ISTD group without `Final Conc.`; empty value cells |
| `wide_one_row.txt` | SCIEX OS analytes horizontally | UTF-8, LF, tab | One header row with one column per analyte; an empty cell |
| `windows1252_ugl.csv` | Any export saved as Windows-1252 | Windows-1252, CRLF, comma | `µg/L` (byte `0xB5`) and an en dash (byte `0x96`) |
| `utf8_bom.csv` | Any export saved with a byte-order mark | UTF-8 with BOM, CRLF, comma | The BOM is stripped and decides UTF-8 |
| `utf16le_bom.txt` | Windows "Unicode text" | UTF-16LE with BOM, CRLF, tab | Refused as `undecodable_text` |
| `ragged_row.csv` | A cut or edited export | UTF-8, LF, comma | One record with a field missing (line 3) and one with an extra field (line 4) |
| `preamble_short.csv`, `preamble_long.csv` | Chromeleon report export | UTF-8, CRLF, semicolon | The same header after 2 and after 12 preamble lines; the long preamble holds blank lines, a partial header line and an unclosed quote |
| `footer_after_blank.csv` | Any export with a footer | UTF-8, CRLF, comma | Two data rows, a blank line, then two footer lines |
| `quoted_fields.csv` | SCIEX quoting of names with commas | UTF-8, CRLF, comma | An embedded comma, a doubled quote and an embedded line break in quoted fields |

`crates/lis-results/tests/file_fixtures.rs` reads every file and checks its rows. Larger and limit cases (50,001 rows, 513 columns, a 4 KiB + 1 field, a signature beyond line 50) are generated in the tests, not stored.
