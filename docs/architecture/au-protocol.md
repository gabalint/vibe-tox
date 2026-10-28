# AU400/AU640 instrument interface

The `lis-au` profile implements the **Olympus AU400 & AU640 Online
Specifications** (ENG-240 in the [engineering source ledger](../research/sources-engineering.md));
page references below cite that document. This profile does not assert compatibility with
other AU models or firmware versions without their parameter sheets and fixtures.

## Contract and implementation boundaries

| Boundary | Implemented behavior | Evidence / requirement |
| --- | --- | --- |
| Wire codec | `lis-au`: incremental bounded framing, binary XOR BCC, optional BCC, one/two control-byte delimiters, optional ETB, 256/512/1024-byte block limits, explicit field widths | Specification pp. 4–7, 33–36; REQ-INT-009 |
| Transport | The same AU codec on a serial port or a TCP listener for a transparent serial bridge. The bridge must initiate the connection and forward unchanged bytes; no Telnet/RFC 2217 or ASTM wrapper | User's requested topology; specification defines RS-232C only |
| Serial | 2400/4800/9600 baud, 7/8 data bits, configured parity and stop bits, no software or hardware flow control. Physical wiring remains a lab commissioning check | Specification pp. 4, 29 |
| Protocol selection | AU and ASTM are explicit connection choices. No heuristic protocol detection and no synthetic conversion to ASTM records | REQ-INT-001/009 |
| Class A | Receive without ACK/NAK; persist complete frames. Missing transmissions cannot be recovered by an acknowledgement mechanism | Specification pp. 4, 18 |
| Class B | ACK only after frame persistence; final sample ACK also follows artifact sync, receipt completion and outbox commit. Earliest response is 500 ms after receiving the frame | Specification pp. 19–22; REQ-INT-009 |
| Blocking | Repeated sample header must agree; blocks 0–9 followed by E, or a single E. Duplicate intermediate blocks do not acquire a second effect. Skipped numeric blocks, changed duplicate blocks and interrupted samples are refused | Specification pp. 7, 26; REQ-INT-003/005 |
| Timeouts | Explicit T2 frame and T1/T7 inter-block timers, 100–9900 ms in 100 ms steps. Starting a frame replaces the inter-block timer with the frame timer. Idle time between samples does not abort a real-time run | Specification pp. 20–22, 26 |
| Receipt provenance | Exact wire frames, checksum and delimiters retained; immutable wire profile saved with each receipt. Recovery and delayed import read this profile, never today's connection settings | REQ-INT-005/009, REQ-OPS-013 |
| Authorization and audit | Reuse the instrument principal, configuration permissions, transactional audit, artifact store, outbox, source-IP allowlist and startup reconciliation | REQ-OPS-001, NFR-08/18/32 |
| Application messages | Parse RB/RE, R-space/RH, DB/DE, D-space/DH, d-space/dH, DR/DA/DQ and host S-space/SH/SE. Classification is case-sensitive | Specification pp. 8–14 |
| Requisitions | Encode caller-supplied normal/rerun requisitions, including whole-item blocking, and session stop. Host S messages omit Unit No. as shown in the supplied diagrams. Automatic LIS order lookup/dispatch is not enabled | Specification pp. 9, 23–25 |

The supported character profile is printable ASCII. Extended Japanese character
ranges conflict between the specification's tables and require separate
qualification. Sample-ID widths 1–20 are supported; zero-width identifiers are
refused because this workflow matches the LIS specimen barcode. The ambiguous
rack-omitted nonpatient layout is also refused; its raw well-framed message remains
available in a failed semantic-import receipt.

## Source qualification and result semantics

Connection settings describe bytes. An **approved AU source registration** separately
declares the exact protocol profile, the local test-code-to-analyte map and the unit
of each explicitly numeric assay. Approval rechecks that the instrument still has
the same wire settings. A changed setting requires a matching source approval;
an old receipt retains its old settings and cannot be silently reinterpreted.
The instrument connection screen configures AU wire settings. The Bench setup
source dialog registers AU sources through `register_source` with an `auProfile`:
it restates the connection's wire settings read-only, fixes barcode matching and
no correction marker, and asks for each numeric test's two-digit code, analyte
and declared unit.

| Wire fact | Import behavior | Requirement |
| --- | --- | --- |
| Sample ID is fixed width | Raw identifier retained. Remove trailing space padding only for exact matching to an unambiguous active run position; never match by analyzer sequence number, rack or patient demographics | REQ-ANA-003, REQ-INT-004 |
| No units are transmitted | Raw measurement units stay absent. The typed result uses the explicitly declared unit in the approved AU profile, which must equal the approved method's unit | REQ-RES-001, NFR-10 |
| No sender version, measurement time, operator or final/correction status | Preserve absence. Source-registration names/version describe setup evidence, not wire assertions | REQ-INT-005 |
| Numeric value with fixed padding | Preserve raw bytes and scale. Parse a separate lexical view after removing space padding, including the documented padding after a minus sign. No implicit decimal scale or unit conversion | Specification pp. 14–17; NFR-10 |
| All-9 field | Refuse as overflow, including the documented six-digit negative-OD overflow; never create a numeric 999999 measurement | Specification p. 17 |
| Two flag characters | Preserve exactly. Any nonblank flag holds the patient row for reconciliation; no flag becomes a toxicology interpretation or automatic correction marker | Specification pp. 30–31; REQ-RES-002/004 |
| LIH packed indices | Exclude from the numeric whitelist. An unmapped LIH code is quarantined. No assumed universal LIH test number | Specification pp. 15–17; REQ-INT-004 |
| DQ, DA, DR | Cross-check nonpatient role against the locally assigned position before routing. A label collision cannot turn QC/calibration/blank into a patient result | REQ-ANA-003/004 |
| DH/dH rerun | Preserve the rerun classification. Changed data uses the existing conflict/reprocessing review; no manufactured status C and no released-report overwrite | REQ-RES-004/005 |
| Dilution execution | Hold diluted executions for reconciliation; this AU profile does not qualify dilution handling, even when a source declaration exists. Never infer a dilution factor from rerun classification | REQ-ANA-006 |

## Replays and failure boundaries

One complete sample is one result receipt. DB/DE and query/session messages are
separate durable receipts, without result-import jobs. A query is retained and
shown as requiring unsupported automatic ordering; no empty requisition or SE is
invented as a negative per-sample response. The manual documents SE as stopping a
session and provides no verified per-sample no-order reply. Automatic ordering
also needs exact accepted specimen/run identity, eligible ordered test mapping,
rerun provenance and cancellation/retry behavior.

The wire protocol has no globally unique message ID, measurement timestamp, total
block count or numbered final block. This constrains what can be proven:

| Case | Handling / remaining limit |
| --- | --- |
| Complete identical single-block sample resent | A new receipt is retained and linked to its prior identical message under the same frozen AU profile; no second import job. Correcting a wire profile permits an independently parsed resend of the same bytes |
| Last block of a multiblock sample repeated | ACK the already durable final block while it remains the last exchange; do not import a tail as a new sample. Across reconnects or intervening exchanges, require a complete replay from block 0 |
| Same numeric block ID, changed bytes | Abort and retain evidence; require retransmission of a complete sample |
| Checksum/framing error during a class-B sample | NAK after the minimum turnaround; retain the durably acknowledged prefix while awaiting a valid retransmission. Bound retries and waiting time; exhaustion aborts |
| Sample timeout, disconnect or class-A transport error | Abort partial evidence. A durable index of multiblock identities refuses a tail after connection/app restart; require a new block 0 and complete replay |
| Same identity later sent as a genuinely single-block sample | If that identity previously used intermediate blocks, conservatively hold the final-only message. The unnumbered E cannot distinguish this from a retry tail; obtain a complete multiblock re-export or reconcile explicitly. Connection-setting changes do not erase this guard |
| Instrument skips an entire intermediate block without any observable bad frame and sends final E | The missing block cannot always be inferred from E. Qualify the instrument's error-control setting and reconcile expected tests against received tests; transport ACK cannot prove a complete performed panel |
| Crash after persisted final frame, before completion | Recover only a fully parseable coherent sample using frozen settings, sync the exact artifact and enqueue import |
| Incomplete or ambiguous durable frames at startup | Mark interrupted; preserve frames and do not import a partial sample |
| Disk/transaction failure | Withhold ACK; report persistence failure. A successful connection does not imply successful clinical import |

No real analyzer, serial hardware, bridge, firmware or laboratory assay is qualified
by the synthetic fixtures. The [validation plan](../quality/validation-plan.md)
and [open qualification gates](../quality/open-items-g1-g2.md) require commissioning
on the actual supported instrument profile before clinical use.
