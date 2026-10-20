# Restore and recovery

Use this procedure to restore the laboratory onto a replacement or rebuilt computer. The in-app
restore is intentionally available only before any user account exists. It does not overwrite an
active laboratory installation.

The current software has been exercised with synthetic data. Replacement-host restore has not yet
been verified on real supported hosts of every operating system, and signed installers are not
available. A laboratory must rehearse and approve its own procedure before relying on it.

## Before restoring

1. Declare the incident under the laboratory's procedure. Preserve the failed computer, logs,
   analyzer records, worksheets, issued reports and delivery evidence needed for reconciliation.
2. Prepare a replacement computer that meets the [host baseline](installation-and-first-start.md#prepare-the-laboratory-computer).
   Enable full-disk encryption before restoring patient data.
3. Install a version of the application that can open the backup. The staging check refuses a
   backup whose database schema is newer than the installed application.
4. Obtain the newest trustworthy recovery kit, an off-host backup target and, when available, a
   separately held anchor folder.
5. Do not create an administrator account on the replacement installation. A restore is refused
   after any user exists.

Cloud device revocation and re-enrollment do not apply because cloud services are not built.

## Restore the backup

1. Open the new installation. On **Set up this laboratory**, choose **Restore from a backup**.
2. Under **Installation key**, type the key exactly as printed in the recovery kit and choose
   **Continue**. Compare the displayed key fingerprint with the kit. The key stays in memory for
   this restore only, and the restore session expires after 30 minutes.
3. Choose **Choose the backup folder** and pick the folder that holds the backups (the folder
   chosen as the backup target, or its `VibeToxLIS` subfolder). Select a backup in the list,
   then choose **Check this backup**. A backup that is not compatible with this version of the
   application cannot be checked.
4. Wait for the full check, which can take several minutes. Review the facts (backup time, app
   version, database size, stored files, database integrity and the **Lost interval until**
   time), the audit-trail table, the anchor counts and the findings. Work after the backup time
   is outside this copy and belongs to the lost interval.
5. Optionally choose **Compare with anchors on another drive** and pick a folder of anchors held
   separately from the backup. The engine checks the backup again with those anchors and refuses
   the restore if they disagree with it.
6. Continue only when **Restore this backup** is offered and the findings are understood. If it is
   not offered, choose another backup or **Cancel restore**. In the dialog, type `RESTORE` in
   capital letters in **Confirmation** and choose **Restore and close the app**.
7. The application stages the restore and closes. Open it again by hand. It does not relaunch
   itself.

**Cancel restore** ends the session and removes what was staged; nothing is changed until step 6.

The setup wizard and checks implement the process in
[restore and reconciliation contract](../architecture/operations-contract.md#restore-and-reconciliation).

## What happens at the next start

Before opening the database, the application checks the staged files again. It moves the current
database aside under the operations `replaced` area, moves the restored database and stored files
into place, then opens them. An interrupted apply is resumable at the next start.

After opening the restored database, the application records the restore, raises accession and
container number floors, and opens a recovery reconciliation. If it cannot record that gate, it
refuses to start with `restore_record_failed`; do not use that computer for laboratory work until
the failure is resolved.

The application then performs startup reconciliation, starts analyzer listeners and records the
new application run. Reports and other outward effects remain held as described below.

## Why outward work waits

Restoring an older snapshot can remove knowledge of actions that happened after that snapshot,
even when recipients, analyzers, paper labels or billing systems still have evidence of them. Until
the recovery reconciliation closes:

| Held action | Why it waits |
| --- | --- |
| Report release | A newer report, cancellation, hold, review or configuration decision may have existed in the lost interval |
| Delivery attempts and delivery PDF exports | A recipient may already hold a newer revision; an older report must not be sent as current |
| Billing handoff | A handoff may already have occurred or later performed work may be absent |
| Label printing | Physical labels issued in the lost interval may carry numbers the restored database no longer knows |

Accessioning, analyzer intake and review can continue, but the laboratory must control that work
under its recovery procedure. This gate is the current conservative default in
[M4-L7](../quality/laboratory-policy-inputs.md#operations) and
[restore and reconciliation contract](../architecture/operations-contract.md#restore-and-reconciliation).

## Work the recovery checklist

Open **Recovery reconciliation**. A quality manager closes it only after every item is checked,
attested and supported by the laboratory's records.

| Checklist item | What to confirm |
| --- | --- |
| This computer is the production computer | The replacement is authorized to resume production. Never attest this on a test copy. |
| Lost interval listed | Record the time from the backup set through the incident or cutover. Work in this period is not in the restored database. |
| Number floors checked | Confirm the floors are higher than every accession and container number on a physical label. Raise them if needed. |
| Analyzer results resent | Use analyzer retention and the **Results expected** queue to identify and resend missing results. |
| Reports reconciled | Compare released and delivered reports from the lost interval. Never resend an older revision over a correction. |
| Orders and work re-entered | Re-enter orders, decisions, QC and reviews that are absent from the restored copy. |
| Billing handoffs reconciled | Compare handoffs and outcomes so nothing is omitted or duplicated. |
| Account changes reapplied | Reapply later disables, role changes and credential actions. A person disabled after the backup may be active again. |
| Configuration changes reapplied | Reapply later catalog, instrument, destination, settings and backup-target changes through normal versioned actions. |
| Audit anchors reviewed | Classify the comparison as matched, expected lost interval, mismatch or not checked; investigate unexplained findings. |
| Waiting labels reviewed | Review the held label-job count and the physical work before allowing the jobs to print. |

Any signed-in session can read the page. A quality manager chooses **Close recovery reconciliation**,
ticks every item under **Items checked**, enters a **Note**, confirms their identity with their password or security key, and chooses
**Close reconciliation**. The engine refuses the closing unless every item is ticked. Closing
releases the gates and lets held label jobs print. The current checklist is defined in
[restore and reconciliation contract](../architecture/operations-contract.md#restore-and-reconciliation).

## Number floors

At restore, the software sets floors above the highest numbers in the restored copy:

| Number | Current skip | Purpose |
| --- | --- | --- |
| Accession number | 1,000 | Avoid reissuing accession numbers printed during the lost interval |
| Container code | 5,000 | Avoid reissuing container barcodes printed during the lost interval |

These are conservative defaults from
[M4-L8](../quality/laboratory-policy-inputs.md#operations), not a
guarantee that they cover the laboratory's lost interval. A quality manager can choose **Raise
number floors**, enter a new accession floor, a new container floor or both (a blank field keeps
that floor), and give a reason. The engine only lets a floor go up. Compare against physical
labels and external records first.

## Audit anchors

Anchors from off-host or separately controlled media help distinguish an expected lost interval
from alteration or truncation. An anchor later than the chosen backup may legitimately show work
that the restore will lose. An anchor that should be present in the backup but does not match is a
finding to investigate.

Use independently held anchors when possible. Anchors copied from the restored database itself
show internal consistency, not resistance to a privileged rewrite. See
[audit seals and anchors](../architecture/operations-contract.md#audit-seals-and-anchors).

## Installation-key panel

If the operating-system key store loses the installation key, the sign-in screen shows
**Installation key needed**. Enter the key from the recovery kit. The engine checks that it opens
the stored sign-in secrets before reporting success.

If the key is missing or mismatched, listeners and work that does not require sign-in can continue,
but users cannot complete TOTP sign-in and backups cannot be encrypted. If the key store itself is
unavailable, ask the person responsible for the host to correct that operating-system problem.
Key behavior is defined in [key and recovery-kit contract](../architecture/operations-contract.md#keys-and-recovery-kit).

## Test copy versus production

| Use | Recovery reconciliation | Outward activity |
| --- | --- | --- |
| Production replacement | Complete every checklist item, then let the quality manager close it | Release, delivery, billing handoff and held label printing resume only after closure |
| Restore rehearsal or investigation copy | Leave the reconciliation open and never attest that it is the production computer | Do not connect production backup targets, destinations, analyzers or printers; do not use it for clinical work |

The software's reconciliation gate is a safeguard, not complete isolation of a test computer.
Keep a test copy off production networks and targets. The default rule that a test copy never
closes reconciliation is documented in
[restore and reconciliation contract](../architecture/operations-contract.md#restore-and-reconciliation).

### Security keys after restore

A restored database carries its saved account methods and key registrations. Reconcile lost or revoked
keys and changed recovery codes since that backup before resuming normal operation: an old backup
can restore an earlier credential state. Verify sign-in with the intended registered keys on the
replacement host and keep the recovery reconciliation open until account changes are checked. The
installation kit restores encryption material; it does not replace a personal key or recovery code.
