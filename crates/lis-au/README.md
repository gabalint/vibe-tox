# Olympus AU400/AU640 codec

Pure protocol handling for REQ-INT-003, REQ-INT-005 and REQ-INT-008/009 boundaries. The caller owns transport, timeouts, authorization, durable receipts, audit, quarantine, laboratory mappings and clinical result acceptance. This crate never sends an ACK or infers a requisition.

Evidence: the user-supplied *AU400 & AU640 Online Specifications*, Olympus Optical Co., Ltd., combined issue **1999-02-03**, reviewed **2026-10-14**. This repository contains original synthetic fixtures and implementation, not a copy of the manual. Printed page numbers are used below.

| Implemented behavior | Manual evidence |
| --- | --- |
| Configurable start/end codes, optional XOR BCC, class A/B | pp. 4–5 |
| Wire limits 256/512/1024; block IDs 0–9 and E, conditional ETB | pp. 6–7, 36 |
| RB, R-space, RH, RE query messages | p. 8 |
| S-space and SH explicit TRI; SE stops the session; no Unit No in S messages | pp. 9–10, 24 |
| D-space, DH, d-space, dH patient result messages | pp. 10–12 |
| DR, DA, DQ kept as distinct nonpatient classifications | pp. 12–13 |
| Rack width 0, 4 or 5 with a two-byte cup only when rack is present; sample ID 1–20; six demographic fields of width 0–20; value width 6 or 9; two flag bytes | pp. 14, 36 |
| Unknown/all-9/LIH values and every flag retain exact bytes | pp. 14–17, 31–32 |

Every raw sample field, result, flag, test code and wire frame uses `lis_core::Phi`, including nested Debug output. Errors contain no received content. Fixed-width values retain spaces and leading zeroes. There are no numeric conversions, units, flag meanings, medical interpretations or implicit status transitions.

`StreamDecoder` accepts arbitrary chunks, retains at most the configured wire limit, and handles standalone ACK/NAK as events. Event output is proportional to the chunk supplied; callers should use bounded reads. `finish` refuses incomplete frames. `ResultAssembler` accepts one sample at a time, requires matching repeated headers, consecutive numeric blocks and final E, and recognizes only an identical last intermediate block as retransmission. It retains at most ten intermediate blocks; `finish` refuses incomplete samples. Errors leave assembly available for explicit caller quarantine/reset.

An identical final E frame may be either retransmission or a legitimate repeated measurement. The manual gives no occurrence identity; the codec deliberately reports each completed occurrence. Durable receipt/session identifiers and reconciliation must resolve this ambiguity. A final E is valid without any preceding numeric block. Numeric blocks do not have an encoded total, so E cannot reveal a lost final intermediate block; class-B receipts/ACKs and transfer qualification remain necessary.

Known unsupported/uncertain profiles are explicit:

| Profile | Behavior and reason |
| --- | --- |
| Delimiters containing ACK (06) or NAK (15) | Rejected to keep idle acknowledgement events unambiguous. Other one/two-byte control sequences, including matching start/end prefixes, are supported. |
| Extended text bytes | Rejected. p4 says A1–DF but p34 gives A1–DC; the codec accepts printable ASCII 20–7E. |
| Sample ID width 0 | Rejected. p14 specifies 1–20 while p36 allows 0; barcode identity is unavailable at 0. |
| Nonpatient result with rack width 0 | Rejected. pp12–13 describe dummy rack fields when rack is absent without unambiguously specifying their width relative to p14's omitted-rack format. |
| More than ten intermediate blocks | Rejected. p7 defines 0–9 and E with no wrap rule. |
| Calibration first/second-data annotation | Not interpreted. The field association is unclear on p12. Exact values and flags remain available for qualification. |

TRI encoding requires all fixed-width header and demographic data explicitly, including reserved/original sample-number fields, and at least one test. Whole test items are split across frames; each repeats the header, and the final frame uses E. Unit No is omitted exactly as in the S diagrams. Empty requisitions are refused because the manual does not qualify them as per-sample no-order replies; no-order behavior is not inferred from SE.
