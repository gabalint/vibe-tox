# Interfaces

Status: **implemented on synthetic data; first delivery step**. The Interfaces menu sends billing handoff files to outside systems through a folder (REQ-INT-001–003, REQ-RPT-003, REQ-RPT-011–013, REQ-BIL-002). The built-in help, Interfaces topic, describes every field and action; this guide covers setup, behavior and recovery. Analyzers are configured separately, under Laboratory setup › Instruments.

## What is available

| Message type | Made when | Direction | Protocol |
| --- | --- | --- | --- |
| Billing JSON (`bil-handoff-v2`) | A billing handoff is created | Out | Folder on this computer or on the lab network |

HL7 orders and results, report PDFs, SFTP, MLLP, code mappings and per-partner interface contracts with acceptance tests are later steps. They are not available and no setting enables them.

## Set up a connection

Everything needs Manage configuration.

1. **Interfaces › Connections › Add connection.** Name the connection and its partner, choose Billing JSON, and **Choose folder…**: the folder comes from the system dialog and cannot be typed. **Test folder** writes and removes a probe file.
2. Add **routing rules** when only some handoffs belong to this partner. Choose the **schedule** and **file name** (`{handoff}` required, `.json` ending). **Save as stopped**.
3. On the connection's page, **Send test message** writes a synthetic file with no patient data. Ask the partner to confirm it.
4. A second person uses **Record destination check** and records how the folder was confirmed. The laboratory's separation-of-duties policy decides whether the person who chose the folder may record it ([ADR 0015](../architecture/decisions/0015-separation-of-duties-policy.md)). The partner must be covered by a business associate agreement; that is the laboratory's responsibility.
5. **Start** the connection, with a reason.

Choosing another folder later needs a new destination check before patient data goes there.

## How messages travel

- **Routing.** Creating a handoff makes one message, in the same transaction, when at least one Billing JSON connection exists. It goes to every connection whose rules all match. A rule matches only if every value of its field in the handoff meets it, so a handoff covering two clients never goes to one client's partner alone. With no rules, a connection receives every handoff. Without any Billing JSON connection no message is made, and files are exported by hand from the handoff page.
- **Writing.** While the app is open, at the sign-in screen too, the LIS looks every 10 seconds for messages it may write. A message is written only while its connection is started, a destination check covers its current folder, its schedule allows it, and no recovery reconciliation after a restore is open. The file is written under a temporary `.tmp` name, synced, and only then given its final name. An existing file is never replaced: a name taken by other content gets `-1`, `-2` before `.json`.
- **Retries.** A failed write is retried after 1, 2, 4 and 8 minutes. After the fifth failure the message waits for **Send again** in **Logs › Failed** and appears in the **Interface messages not written** work queue. Sending again uses the same stored file and never makes a new message or report.
- **Evidence.** Logs show what the folder can prove: **Written**, **Failed** or **Not sent**. A folder never confirms that the other system used the file, so a written file shows **Outcome unknown**, never Delivered. The billing owner's acknowledgement or rejection is recorded on the handoff (REQ-BIL-002). Each write attempt is an audit event on the security stream; connection changes, destination checks, unrouted decisions and log exports are audited too.

## Unrouted messages

A handoff that matched no connection waits in **Interfaces › Unrouted messages** and in the **Unrouted interface messages** work queue; the menu shows the count. Its panel shows the connections it was checked against and the closest rule. Choose one decision:

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
| Connection shows **Error** | Overview names the latest problem. Restore the folder or its permissions, use **Test connection**, then **Send again** on failed messages |
| Messages stay **Not sent** | The row's details say why: connection stopped, destination check not recorded, waiting for the schedule, or written at the next pass |
| The app was closed | Nothing is written while it is closed. Waiting messages are written after it starts again |
| A restore was applied | Writing is held until the recovery reconciliation closes |

## Limits

- Connections are stopped, not deleted, so their history and messages remain.
- Message content is not shown in Logs. The handoff file is opened from Billing › Billing handoffs, by people with Billing, as an audited read.
- The folder's network share, permissions and the partner's process for reading files are the laboratory's to qualify. Passing tests on synthetic data is not qualification.
