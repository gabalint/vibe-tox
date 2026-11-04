# Interfaces

Status: **implemented on synthetic data; third delivery step**. The Interfaces menu receives HL7 orders from a clinic's EMR and sends HL7 results, issued report PDFs and billing handoff files to outside systems, through a folder or the partner's SFTP server (REQ-INT-001–004, REQ-ORD-001, REQ-RPT-003, REQ-RPT-011–013, REQ-BIL-002). The built-in help, Interfaces topic, describes every field and action; this guide covers setup, behavior and recovery. Analyzers are configured separately, under Laboratory setup › Instruments.

## What is available

| Message type | Made when | Direction | Protocol | Default file name |
| --- | --- | --- | --- | --- |
| HL7 Results (ORU^R01, v2.5.1) | A report revision is released; a withdrawal message when the report is withdrawn | Out | Folder, or the partner's SFTP server | `{accession}_r{revision}_{date}.hl7` |
| Report PDF | A report revision is released; its withdrawal notice when the report is withdrawn | Out | Folder, or the partner's SFTP server | `{accession}_r{revision}_{date}.pdf` |
| Billing JSON (`bil-handoff-v2`) | A billing handoff is created | Out | Folder, or the partner's SFTP server | `billing-{handoff}_{date}.json` |

In a report's file name, `{accession}` is the report number (the accession number and the specimen type code, so two specimens of one accession never share a name) and `{revision}` the revision number; a withdrawal's revision reads `<n>-withdrawn`.

| HL7 Orders (OML^O21, or ORM^O01, v2.5.1) | The clinic's EMR writes a file | In | Folder, or the clinic's SFTP server | — (read, not named) |

A folder is on this computer or on the lab network. HL7 acknowledgement files back to the clinic, MLLP and per-partner interface contracts with acceptance tests are later steps. They are not available and no setting enables them.

## HL7 Orders (incoming)

An HL7 Orders connection reads the order messages a clinic's EMR leaves in an input folder, local or on the clinic's SFTP server, every *Check every* seconds (10–3600) while the app is open, at the sign-in screen too. Its partner is one of the laboratory's clients: every order it receives is for that client. It reads only once started with a destination check that covers its input folder or server, and never while a recovery reconciliation is open.

- **Reading.** Only complete files are read: `*.hl7` (any case), not hidden, never `*.tmp`; at most 20 per reading. The bytes are stored first; then one transaction records what the message became, with the order it made; then the file moves to Processed or Failed › year › month (such as `Processed/2026/11/clinicA_000124.hl7`), never replacing a file. A file read again after a crash before its move is only moved.
- **Reader.** The LIS's own narrow reader ([ADR 0007](../architecture/decisions/0007-integration-edge-and-interface-engines.md) §7) reads MSH, PID, PV1, ORC, OBR and DG1, as supplied. Only new orders (ORC-1 `NW`) are read. The placer group number (ORC-4) names the requisition, else the first ORC-2. It claims no profile conformance.
- **Accepting.** A message whose MSH-4 is not the connection's *Accept messages from* is refused. The ordering provider must be a registered, active practitioner by NPI (ORC-12, else OBR-16); PV1-3, if given, a location of the client; each OBR-4 an order code with an approved code mapping, which gives the LIS test and its sequence role (first line or confirmation) as the partner's contract defines it (REQ-INT-004). A diagnosis (DG1-3) is kept as supplied and refused without its coding system.
- **Patient.** The patient is found only by a PID-3 identifier, by assigning authority (else the sending facility) and type; the LIS never creates a patient from a message. With no match the message waits (or is refused, per *Unknown patient*) and becomes an order at the first reading after the patient is registered with that identifier. Two matching patients, or a registered patient whose family name or birth date differs, hold it for review.
- **Order.** The order is registered in Received status with source `HL7 interface (EMR)`, the placer number and authority, the stored message as its source document, and evidence as supplied: no signature evidence (the laboratory's order policy decides at acceptance, REQ-ORD-005), billing responsibility not supplied, no medications. The laboratory accepts or rejects it in Orders as any order.
- **Replays (REQ-INT-003).** An exact replay (the same bytes) is a duplicate with no effect. The same MSH-10 with other content is held for review. A placer number that already has an order is refused.
- **Logs.** Each message shows Dir. In, transport Received, and what it became: Order created, Waiting for patient, Held for review, Duplicate – ignored or Rejected with its reason, and where its file moved. The LIS writes no acknowledgement file back; the clinic learns the outcome by the laboratory's agreed route.

## SFTP

The LIS connects out to the partner's SFTP server ([integrations](../architecture/integrations.md)); it never accepts connections. The client is russh with the ring backend, admitted in the [library policy](../governance/library-policy.md#sftp-client-admission--2026-11-04).

- **Server identity.** The connection pins the server's key by its SHA-256 fingerprint, as OpenSSH prints it (`SHA256:` and 43 characters). The LIS refuses any other key, and Test connection shows the fingerprint the server presented. Get the fingerprint from the partner's IT through a channel the laboratory trusts. The server must offer an Ed25519 or ECDSA host key; RSA is not supported (the `rsa` crate is excluded, RUSTSEC-2023-0071).
- **Sign-in.** SSH key (recommended): one Ed25519 key per installation, made on the first **Copy LIS public key**. Its private key is sealed with the installation key in the database and never leaves the engine; the partner's IT allows the public key (`ssh-ed25519 … vibetox-lis`) for the username. Password: sealed with the installation key, never returned to the screen, and kept on a change only while the server, port and username stay the same. Either needs the installation key on this computer; without it, attempts fail with `credential_unavailable`. A restore that brings the installation key brings both.
- **Writing.** Each file is uploaded under a temporary `.tmp` name in the remote folder and renamed when complete. An SFTP rename never replaces a file: a name taken by other content gets `-1`, `-2`; a name already holding the same bytes is taken as the earlier write of the same message. Connecting and each step without an answer time out after 20 seconds, then the usual retries apply.
- **Destination check.** It covers the server, port, username, remote folder and fingerprint together. Changing any of them needs a new check; changing the sign-in or the password does not.
- **Problems** in Logs and on Overview: `server_unreachable` (no answer, refused, or an RSA-only server), `fingerprint_mismatch`, `sign_in_failed`, `credential_unavailable`, and the folder codes `folder_missing`, `access_denied`, `name_taken`, `write_failed`.

## HL7 Results messages

The LIS's own narrow encoder writes the message (no HL7 library is bundled; [ADR 0007](../architecture/decisions/0007-integration-edge-and-interface-engines.md) §7). It claims no LRI conformance: the partner's contract and acceptance tests decide that, and the profile choices below are defaults to confirm with each partner.

| Segment | Content |
| --- | --- |
| MSH | Sending application `VIBETOX LIS`, the laboratory's name and CLIA number, the partner as receiving facility (MSH-6), the delivery ID as control ID, processing ID `P` (`T` for the test message), no acknowledgement requested (`NE`), UTF-8 |
| PID, PV1 | The patient's identifiers, name, birth date and administrative sex as recorded; the client location (PV1-3), patient class `U` |
| ORC, OBR | One group per ordered test with a reported line: the ordering source's order number, a filler order number stable across revisions (report number and test position), the partner's test code (OBR-4), collection time, ordering provider by NPI, ordering facility (ORC-21), release time, status `F` or `C` for a correction |
| OBX | One per reported analyte: the partner's code and the LOINC code (the mapping's, else the catalog's), the value exactly as stored (NM), a censored value as comparator and number (SN), a qualitative code as received (ST), no value with status `X` when no result could be obtained; the mapping's UCUM unit, the reference interval if configured, the performing laboratory. No abnormal flag or interpretation is sent (REQ-RPT-008) |
| SPM | The accession number, the specimen type, collection and receipt times |

A withdrawal is sent as the withdrawn revision's results with OBX status `W` (posted in error) and OBR status `C`; confirm with each partner that its system removes them.

**Code mappings (REQ-INT-004).** Each HL7 connection maps LIS tests and analytes to the partner's codes, with LOINC and the UCUM unit. Changes are drafts on the connection's Code mappings tab; a second person approves them as a new version with a reason, or the person who drafted them where the separation-of-duties policy allows ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)). The message is made at its first write attempt with the version then in force; its bytes and the version are kept, so every retry writes the same bytes and a later version never changes a written message. A message whose test or analyte has no approved mapping, or whose quantity's unit is not the mapping's unit, is held: never sent with a guessed code or a converted unit. The tab lists what held messages need.

## Set up a connection

Everything needs Manage configuration.

1. **Interfaces › Connections › Add connection.** Name the connection and its partner and choose the message (HL7 Results, Report PDF or Billing JSON). For HL7 Results, the partner's name is sent as the receiving facility (MSH-6).
   - **Folder:** **Choose folder…**: the folder comes from the system dialog and cannot be typed. **Test folder** writes and removes a probe file.
   - **SFTP:** enter the server, port, username, server fingerprint and remote folder. With SSH key sign-in, **Copy LIS public key** and send it to the partner's IT; with password sign-in, type the password. **Test connection** connects, checks the fingerprint and sign-in, and writes and removes a probe file. Nothing is saved by either.
2. Add **routing rules** when only some reports or handoffs belong to this partner. Choose the **schedule** and **file name** (the message type's required placeholders and extension). **Save as stopped**.
3. On the connection's page, **Send test message** writes a synthetic file with no patient data: an ORU^R01 with processing ID `T` for HL7 Results, a one-page PDF for Report PDF, a JSON file for Billing JSON. Ask the partner to confirm it. For HL7 Results, add the partner's codes under **Code mappings** and have a second person approve them.
4. A second person uses **Record destination check** and records how the folder or server was confirmed. The laboratory's separation-of-duties policy decides whether the person who chose the folder may record it ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)). The partner must be covered by a business associate agreement; that is the laboratory's responsibility.
5. **Start** the connection, with a reason.

Choosing another folder later, or another SFTP server, port, username, remote folder or fingerprint, needs a new destination check before patient data goes there.

## How messages travel

- **Routing.** Releasing a report revision makes one message of each report type (HL7 Results, Report PDF), and creating a handoff one Billing JSON message, in the same transaction, when at least one connection of the type exists. An HL7 rule names its field by the HL7 field that carries it (ORC-21, OBR-16, PV1-3, OBR-4) but compares the LIS codes. It goes to every connection whose rules all match. A rule matches only if every value of its field meets it, so a handoff covering two clients never goes to one client's partner alone. A report's values are its order's client, practitioner and location and the tests on its specimen. With no rules, a connection receives every message of its type. Without any connection of the type no message is made, and files are exported by hand as before.
- **Corrections and withdrawals (REQ-RPT-013).** A report's later revision, correction or supplement, goes first to every connection of the same type that had an earlier revision (HL7 with corrected status `C`), even when its rules no longer match, and is written before other waiting messages. An earlier revision not written yet is superseded: it is never written, and an unrouted one leaves Unrouted messages. A withdrawal notice or HL7 withdrawal goes to every connection an earlier revision was written to; with none, no message is made.
- **Impact pause (L13).** A report revision waits, unwritten, while its impact review is open or a correction is due; Logs say so. Withdrawal notices never wait for it.
- **Writing.** While the app is open, at the sign-in screen too, the LIS looks every 10 seconds for messages it may write. A message is written only while its connection is started, a destination check covers its current folder or server, its schedule allows it, and no recovery reconciliation after a restore is open. The file is the stored file, byte for byte: the issued report PDF is never regenerated. It is written under a temporary `.tmp` name, synced (in a folder), and only then given its final name. An existing file is never replaced: a name taken by other content gets `-1`, `-2` before the extension.
- **Retries.** A failed write is retried after 1, 2, 4 and 8 minutes. After the fifth failure the message waits for **Send again** in **Logs › Failed** and appears in the **Interface messages not written** work queue. Sending again uses the same stored file and never makes a new message or report.
- **Evidence.** Logs show what the folder or server can prove: **Written** (**Uploaded** for SFTP), **Failed** or **Not sent**. A folder or SFTP server never confirms that the other system used the file, so a written file shows **Outcome unknown**, never Delivered. The billing owner's acknowledgement or rejection is recorded on the handoff (REQ-BIL-002). A written report PDF is not a staff-attested delivery on the report's page, but a revision that goes to a connection leaves the **Reports without a destination** work queue. Each write attempt is an audit event on the security stream; routing, connection changes, destination checks, unrouted decisions and log exports are audited too.

## Unrouted messages

A message that matched no connection waits in **Interfaces › Unrouted messages** and in the **Unrouted interface messages** work queue; the menu shows the count. Its panel shows the connections it was checked against and the closest rule. Choose one decision:

| Decision | Effect |
| --- | --- |
| Send once | One delivery to the chosen connection, under its usual checks |
| Retry routing | Routes it by the rules as they are now; refused while no rule matches |
| Deliver another way | Records how it was delivered outside the LIS, for reconciliation |
| Dismiss with reason | Leaves it undelivered, with the reason |

**Add a rule** opens the closest connection, where Edit changes its rules.

## Recovery

| Symptom | Action |
| --- | --- |
| Connection shows **Error** | Overview names the latest problem. Restore the folder, the server or the permissions, use **Test connection**, then **Send again** on failed messages |
| SFTP **fingerprint mismatch** | Do not copy the presented fingerprint. Confirm with the partner's IT through a trusted channel whether the server's key changed; if it did, Edit the fingerprint and record a new destination check |
| SFTP **sign-in failed** | Check that the partner allowed the LIS public key for the username, or type the password again |
| SFTP **credential unavailable** | Use Copy LIS public key once to make the LIS key, type the password again, or restore the installation key (System › Recovery kit shows whether it is usable) |
| Messages stay **Not sent** | The row's details say why: connection stopped, destination check not recorded, waiting for the schedule, paused for the report's impact review, held for a code mapping, or written at the next pass |
| An HL7 message is **held** | Map the rows marked Not mapped – held on the connection's Code mappings tab, with the result's unit as the UCUM unit, and have a second person approve them |
| A report revision shows **Replaced by a newer revision** | Nothing to do: the newer revision went to the connection instead |
| The app was closed | Nothing is written while it is closed. Waiting messages are written after it starts again |
| A restore was applied | Writing is held until the recovery reconciliation closes |

## Limits

- Connections are stopped, not deleted, so their history and messages remain.
- Message content is not shown in Logs. Reports are opened from Reports, and the handoff file from Billing › Billing handoffs, by people with those rights, as audited reads.
- The folder's network share, permissions and the partner's process for reading files are the laboratory's to qualify. Passing tests on synthetic data is not qualification.
- The SFTP transport is tested against an in-process server on synthetic files, not against a partner's server. Each partner's server, network path and account are the laboratory's to qualify.
