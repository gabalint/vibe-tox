# Synthetic ASTM conformance corpus

Every `.raw` file contains analyzer-to-host bytes exactly as fed to the pure
receive driver: ENQ, framed bytes, and EOT where applicable. All identifiers and
sender names are synthetic. The adjacent `.expected` file states the observable
outcome asserted by `tests/conformance.rs`; it is deliberately small so the test,
not a second parser in fixture data, owns detailed assertions.
`packed_records.hex` uses hexadecimal so reviewers can see both record
terminators inside its one low-level frame.

| Case | Coverage |
| --- | --- |
| `valid_single_result` | One H/P/O/R/L message and lexical result preservation |
| `multiple_patients_orders_results` | Scoped P, O, and R sequences and tree assembly |
| `record_spanning_etb` | A 500-character comment split over several ETB frames |
| `packed_records` | H and L records packed into one low-level frame |
| `custom_delimiters` | Header-selected field, repeat, component, and escape delimiters |
| `escape_sequences` | F, S, R, E, and even-pair hex escape decoding |
| `comments` | Patient-, order-, and result-level comments |
| `bad_checksum_then_retransmission` | NAK followed by successful retransmission |
| `out_of_sequence_frame` | Unexpected first frame number is NAKed |
| `duplicate_after_lost_ack` | Duplicate durable frame is re-ACKed without another persist |
| `eot_mid_record` | Partial record is aborted and never exposed as a message |
| `oversize_frame` | Text above the selected 240-byte frame profile is rejected |
| `disallowed_byte` | NUL in frame text is rejected by the conservative ASCII profile |
| `missing_terminator_record` | Link transfer completes, then hierarchy validation rejects missing L |
| `empty_transfer` | ENQ followed by EOT aborts without a message |

The standards are licensed publications and no standard text is copied here.
These fixtures exercise the public profile described in the crate documentation;
they do not claim clinical validation or universal analyzer compatibility.
