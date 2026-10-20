# Daily operation

The application is one process with no background service. Keep it open whenever analyzers may
send results. Nobody needs to remain signed in to the application: listeners continue at the
sign-in screen and actions taken later still require a person's account.

## Personal sessions and confirmations

Use your individual LIS account even on a shared desktop. **Signed in as … → Lock now** hides the
workspace while the app and analyzer listeners continue running. Unlock with the same account's
password, selected security key or, when enabled, the optional
[personal unlock PIN](installation-and-first-start.md#optional-personal-unlock-pin). Restart,
sign-out and the 12-hour session limit require normal sign-in. **Sign in as a different user**
ends the session and returns to sign-in; use your own credentials. Idle locking does not sign out
the operating system.

Sensitive actions still require a fresh confirmation; the personal unlock PIN cannot approve them.
Password accounts see **Your password**.
Key accounts see **Security key**: select one of your registered keys, review the action and submit,
then complete the native PIN/biometric and touch prompt. The confirmation applies to that submitted
action and its reviewed values; it cannot authorize a different action. A refusal or changed record
requires reviewing the current state before a new attempt. Key settings and lost-key recovery are
in [the sign-in guide](installation-and-first-start.md#manage-your-keys-and-recover-access).
Whether the laboratory allows keys without a PIN for sign-in and unlock, and what that changes, is
described under [security keys without a PIN](installation-and-first-start.md#allow-security-keys-without-a-pin-for-sign-in).

## Find a patient

**Front desk → Patients** opens a directory of registered patients, 25 per page. Use
**Previous** and **Next** to browse, or type the start of a name or identifier in the find field.
Names ignore case; identifier values are case-sensitive. Full names can be entered as
“Given Family” or “Family, Given”. Earlier names can match, but the table shows current details.

**Detailed search** offers an exact identifier with its assigning authority and type, or a
family name with optional given names and birth date. **Show all patients** clears the search.
Check the name, birth date and identifier before opening a record. Clicking a row or pressing
Enter opens it. Browsing, matching candidates and reading patient identity are audited and
allow order-entry or quality-management authority. **Register patient**, **Orders**, **New order**,
identifier addition and standing-order workflows still require order-entry authority. A quality-only
account can review and correct identity or resolve holds without gaining order-entry access. Patient
billing opens only with order-entry or billing authority. Search text is kept out of URLs and browser storage.

## Correct identity and order evidence

Open the patient record to review current demographics and identifier history. Quality managers can correct demographics after checking matching candidates and supplying source/reason; order-entry users can add an identifier and quality managers can deactivate one. These actions preserve prior evidence. They do not merge duplicate patients. For unknown or partial DOB, use **Incomplete identity intake**: retain the supplied name, birth evidence, source and reason; later resolve against an explicitly selected verified patient or record a decline. No patient or order is created by the intake record. Refresh clears an intake selection so changed evidence is reviewed before disposition.

The order page exposes evidence amendments and cancellation requests/decisions with their history. Amend only supplied evidence and identify superseded entries; cancellation is a request until the permitted decision is recorded. A stale revision requires refresh and renewed review; the app does not silently transfer approval to newer content. Existing report-impact gates remain effective.

## Standing authority and method continuity

Register a standing order from the patient page with the ordering practitioner, source reference, authorized scope, inclusive validity dates and optional positive visit limit. Select it explicitly on a new order for that same patient/practitioner. Practice templates do not authorize testing and selection does not expand ordered scope. Accepted orders reserve a use; cancellation does not replenish it. Testing and release evaluate the supplied collection date, using its original offset for timestamp evidence. Unknown collection or a correction outside validity remains held; obtain accurate evidence or cancel unauthorized testing rather than substituting intake time.

An administrator may approve a new method while previously accepted work remains. Existing orders, runs and repeats retain their original approved version; new orders use the current version. Reopen saved preparation from **Worklist**, and select completed preparation for the correct version when placing or repeating a patient execution. Repeat, re-extraction and dilution require parent execution and reason; dilution factors remain exact decimal text. Explicit method retirement and safety holds still apply.

Method-version and reviewed-catalog approval dialogs accept an optional **External approval reference** (1–1,000 characters when supplied; blank means absent). Use it to identify formal approval kept outside the app. The reference stays with the approved version or review and its acting administrator/time; it cannot be edited after approval. It is not patient-report method text. The configured separation-of-duties policy still applies, and configuration approval grants no order-entry, technical-review or release permission.

## Material and report provenance

**Laboratory setup → Material configuration** configures label profiles and storage locations. The container record exposes movement, holds/releases, depletion, disposal and attributed history; storage excursions have affected-container assessment and history. Printer availability and physical label qualification remain laboratory checks. An unavailable printer must not be represented as a successful physical print.

Reports prepared with template v3 retain the performing-site version pinned to the run. When receipt conditions exist, the preparer must explicitly choose which descriptions and earlier conditions are clinically relevant to the patient report, and record a reason. Current condition codes and disposition remain mandatory. Excluded descriptions, every original receipt record and the selection reason remain internal evidence; the reason is not printed. A changed source requires closing the dialog, refreshing and reviewing again. Changing only the selected report text after issue requires a correction reason and preserves the earlier PDF. An older open candidate without these selections must be prepared again before release. Missing legacy performing-site evidence blocks new preparation until quality records an explicit source-backed resolution; it does not change old issued PDFs. Generation/preparation time is distinct from actual release history. Per-result recorded and first-reported times describe known provenance, not an invented bench-completion time. Corrections identify changed results or context and preserve prior-version relationships and bytes.

## Reach pending work

Use **Load more** or **Load older** on bounded histories and queues. Equal timestamps do not discard records. Pending-work counts may be capped summaries; historical out-of-date and blocked-review counts require opening the queue to check. Review scans show how many candidate specimens have been checked and continue through history in bounded requests. A page with no matches can still have more to check; only completion establishes that nothing is waiting. Pause stops further requests after the current page, resume continues from the checked position, and a failed continuation preserves loaded work for retry. Refresh starts the scan again so changed work before the earlier position can be found. A task opens its exact record, including preservation incident, deviation and billing patient/date group. Refresh rechecks current state; a queue is a changing work list, not a frozen report. Review the new state before resubmitting a refused action.

## Keep the computer available

- Keep the operating-system session signed in. Locking the screen is acceptable; signing out,
  restarting, updating or shutting down stops listeners.
- While an analyzer listener is listening, connected or receiving, the application asks the
  operating system to keep the computer awake without keeping the display on.
- Check **Host health → This computer → Keep awake**. `Unavailable` means the operating system
  refused the inhibitor. Set the computer not to sleep while analyzers send results and resolve
  the host problem.
- Sleep prevention has not yet been verified on real hosts of every supported operating system.
  Modern Standby and battery behavior can still override expectations. See
  [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).

## Close and quit warning

The application checks for work that closing could interrupt:

- active analyzer listeners;
- receipts still being received;
- pending outbox work, named by topic;
- a backup, audit-anchor export, restore test, exit export or integrity verification in progress.

If nothing is pending, it closes. Otherwise a native dialog lists the PHI-free warnings and offers
**Cancel** or **Quit anyway**. Prefer **Cancel**, finish or reconcile the work, and then close.
Choosing **Quit anyway** records the stop and attributes it to the signed-in person or the sign-in
screen. During operating-system shutdown or sign-out, the warning is not allowed to prevent the
shutdown indefinitely. Exact close, quit, sign-out and shutdown behavior still needs real-host
verification under [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).

## Listening gaps

At each start, the application records the prior run and the interval when it was not listening.
When that interval exceeds the current 60-second default, it becomes an unacknowledged listening
gap. It also reconciles interrupted receipts and outstanding expected results. A gap does not prove
that a result is missing; it marks a period the laboratory must check. The default is defined in
[operations records](../architecture/operations-contract.md#ownership-and-interfaces).

For each unacknowledged gap:

1. Open **Work queues → Listening gaps** or **Health history → App runs**.
2. Check analyzer retention, interrupted imports, the **Results expected** queue and the relevant
   worksheets or other approved source records.
3. Ask the analyzer to resend the applicable results through the supported interface. Do not
   transcribe or infer a result merely to clear the gap.
4. Choose **Acknowledge listening gap** and record what was checked.

The analyst capability currently owns gap acknowledgement. Startup and gap behavior is defined in
[host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle) and REQ-OPS-013.

## Host health

The top-bar health indicator shows the worst current check. An `unknown` check counts as a warning,
so a green indicator should not hide something the app could not inspect. **Check now** reruns
eligible checks and is limited to once per minute. The background health evaluation records a
history row when a check changes state.

| Area | What to review | Usual action |
| --- | --- | --- |
| Backups, off-host/offline copies, restore tests and recovery kit | Age, failures, target availability and custody | Follow [Backups and recovery kit](backups-and-recovery-kit.md) |
| Audit anchors and audit-chain integrity | Lag or verification findings | Connect an off-host target; investigate mismatches under the laboratory's incident procedure |
| Stored files and free disk | Artifact problems or insufficient room for safe snapshots | Preserve evidence; free space or move to a larger local disk |
| Disk encryption, OS support and UPS | Host control is off, unknown or nearing end of support | Correct the host or record a permitted manual attestation with evidence |
| Web engine | Version changed outside an application update | Run the laboratory's UI smoke check, then attest it |
| Analyzer listeners and keep-awake | Listener error or sleep inhibitor unavailable | Check connections and host power settings before expecting results |
| Listening gaps and unclean stops | The app was unavailable or did not close normally | Reconcile expected work; check UPS and closing practice |
| Clock | Backward jump, forward discrepancy or time synchronization unknown/off | Stop relying on the displayed time until the host clock and source evidence are reconciled |
| Outbox | Dead letter or old pending external work | Open the owning queue and reconcile the external effect rather than retrying blindly |
| Version | Running version is unapproved or not accepted | Follow [Updates](updates.md) before clinical use |
| Recovery reconciliation | A restore gate is open | Follow [Restore and recovery](restore-and-recovery.md) |
| Cloud services | “No cloud services configured” | Expected; cloud services are not built |

Checks and current default thresholds are in
[host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).
The laboratory may change operations thresholds through versioned **Operations settings**. The
defaults remain laboratory questions M4-L2 to M4-L6 until approved locally
([laboratory policy inputs](../quality/laboratory-policy-inputs.md#operations)).

## Work queues for operations

The **Work queues** page shows only queues the signed-in person's capabilities allow. Operational
queues currently include:

| Queue | Owner | Meaning |
| --- | --- | --- |
| Operations alerts | The capability assigned to each health check | A health check is in warning or alert |
| Listening gaps | Analyst | A period without listening needs review and acknowledgement |
| Recovery reconciliation | Quality manager | A restored installation is waiting for the recovery checklist |
| Version acceptance | Administrator | The running version is waiting for recorded application acceptance; laboratory validation evidence may be maintained externally |
| Results expected | Analyst | An open run position has no measurement and may need analyzer resend or investigation |

Counts and ages come from the engine. An empty queue is not evidence that another role has no work;
the current account may simply lack that queue's authority.

## Clock problems

The host clock supplies audit times. The application compares wall-clock movement with monotonic
elapsed time and with the newest stored audit time. A backward jump or a clock behind existing
records raises an alert for 24 hours. A forward discrepancy above the current 60-second default
records a clock-change audit event; it does not by itself create an item in the **Listening gaps**
queue. Time synchronization that is off or cannot be read is a warning. These defaults come from
[host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).

If the clock is wrong, preserve the displayed evidence, correct the operating-system time source,
and reconcile events whose order or elapsed time may be affected. Offline software cannot compare
itself with true current time.

## What an unclean stop means

An unclean stop means the previous application run had no recorded clean or **Quit anyway** stop.
It can follow a power loss, process crash, forced termination, OS sign-out or reboot. It does not
by itself prove database damage or lost results.

At the next start, the application marks the prior run unclean, records the gap, marks interrupted
operations jobs accordingly, reconciles receipts and starts listeners. Review **Health history**,
the listening gap, backup state, expected results, outbox items and any work that was in progress.
Repeated unclean stops should prompt a UPS and host review.
