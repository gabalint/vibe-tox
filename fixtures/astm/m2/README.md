# Synthetic ASTM import fixtures

Synthetic data only. The sender (`SYNTH-LCMS`), sample labels, test codes, operators and values were made up for tests. They describe no real patient, analyzer, assay or laboratory. The numbers are not assay cutoffs or reference values.

## Format

Each `.astm` file is a receipt artifact in the form `lis-interfaces` stores: the ASTM records of one message, each ended by a carriage return (CR, `0x0D`), with no frames. `lis_astm::parse_message_records` reads it directly, and `lis_results::astm::extract_artifact` turns it into rows. To send a file over a link, split it on CR, drop the empty tail, and pass the records to `lis_astm::encode_records`. Keep the CR bytes when editing: editors that rewrite line endings break the files.

`crates/lis-results/tests/astm_fixtures.rs` checks the extracted rows of every file.

## Setup the files assume

| Item | Synthetic value |
| --- | --- |
| Source registration | Sender `SYNTH-LCMS`, version `1.0`; sample label in O.3 (`specimen_id`) |
| Test codes (R.3, fourth component) | `SYN-MOR` and `SYN-OXY`, each mapped to one analyte of the method version |
| Unit configured for both analytes | `ng/mL` |
| Open run on the sending instrument | Patient positions `C00000001` to `C00000005`; control positions `QC-LOW-SYN` and `QC-HIGH-SYN` |
| Suggested test configuration | Cutoff `50` for both analytes, threshold rule `at_or_above`, `before_rounding`, display scale `1` |

## Files

| File | Header time (H.14) | Content | Expected import outcome ([contract](../../../docs/architecture/order-to-result-contract.md#receipt-and-import)) |
| --- | --- | --- | --- |
| `journey.astm` | `20261012093000` | Low control, three patient samples and high control; two analytes each, status `F`. Patient values: `C00000001` `152.40` and `<5.0`; `C00000002` `50.0` and `49.9`; `C00000003` `>1000` and `0.00` | Control rows go to QC observations, never to patient results. Six patient measurements and result lines. With the suggested configuration: `152.40`, `50.0` and `>1000` are `meets_cutoff`; `<5.0`, `49.9` and `0.00` are `does_not_meet_cutoff` |
| `journey_duplicate.astm` | same | Byte-identical copy of `journey.astm` | The interfaces receipt is marked `duplicate_of`, so no import attempt is created |
| `correction.astm` | `20261012110000` | `C00000001` `SYN-MOR` changed from `152.40` to `148.90`, status `C` | With revision indicator `result_status_correction`: a new processing revision, a superseded measurement and a result revision with cause `reprocessed`. With indicator `none` (the M2-R5 default): quarantined as `conflicting_duplicate` |
| `partial.astm` | `20261012113000` | Only `C00000002`, both rows unchanged | Both rows count as `rows_duplicate`. The other samples' rows are untouched: omission never deletes or means negative |
| `unknown_sample.astm` | `20261012120000` | Sample `C99999999`, which is on no open run | Quarantined as `unknown_sample` |
| `unit_mismatch.astm` | `20261012123000` | `C00000004` `SYN-MOR` in `ug/L`; `SYN-OXY` in `ng/mL` | The `ug/L` row is quarantined as `incompatible_unit` (no conversion); the other row is imported |
| `malformed_value.astm` | `20261012130000` | `C00000005` `SYN-MOR` value `< 5.0` (space after the comparator); `SYN-OXY` `8.4` | The first row is quarantined as `malformed_value` (nothing is trimmed); the other row is imported |
