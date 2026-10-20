# Updates

The application records approval and acceptance of updates, but it does not yet discover,
download, verify or install an update. Signed installers, Minisign-signed update packages and the
Tauri updater wait for the publisher and release-channel decisions in
[Q-19 and Q-34](../planning/open-questions.md#decision-and-discovery-queue).

The current workflow is therefore a controlled record around a separately installed package. It
does not make an unsigned developer build safe for clinical use.

## Before approving

1. Obtain the release package through the laboratory's approved route. Review the change summary,
   published risk class, known limitations, migration notes and available verification evidence.
2. If appropriate, restore a recent backup onto an isolated test computer. Never close the test
   copy's recovery reconciliation or connect it to production targets, analyzers, destinations or
   printers.
3. Check **Backups → Runs** for a recent verified manual or pre-update backup. If needed,
   choose **Request pre-update backup** on **Versions and updates → Update approvals** or
   directly inside the approval dialog.
4. Wait for that run to show a verified copy. A queued or running backup is not enough.

The application accepts a manual or pre-update backup only when it completed no more than
24 hours before approval and still has a verified, unpruned copy. Scheduled backups do not
qualify because automatic retention may remove them. These are application design rules in
[update-record contract](../architecture/operations-contract.md#update-records).

## Record approval

An administrator with configuration authority chooses **Record update approval** and enters:

- **Version to install**, copied from the downloaded update's release notes (package version
  detection is not implemented);
- the published risk class;
- confirmation that the release notes were reviewed;
- **Backup before this update**, selected by completion time and verified destination;
- approval notes;
- identity confirmation using the account's sign-in method.

The backup choices come from the installation's recorded **Backups → Runs** history, not a
scan of files on disk. **Load older backup runs** continues past the initial 100 runs;
**Refresh backup list** checks for newly completed work. The engine determines eligibility
and rechecks it when saving, so a backup that expires while the form is open may be refused.

The application then opens a file dialog for the update package, computes its SHA-256 digest and
records that package with the approval. This step does not verify a publisher signature and does
not install the package.

## Install and start

1. Close the application safely. Do not override listener or pending-work warnings merely to start
   the update.
2. Install the package using the controlled procedure supplied with that release. The in-app
   updater is not built.
3. Open the application. Before applying pending database migrations, it makes an integrity-checked
   local safety snapshot when required. If that copy fails, startup stops rather than migrating.
4. Review startup, host health, listener state, the web-engine change check and the laboratory's
   release-specific smoke tests.

Automated rollback is not built. The planned rollback is reinstalling the prior package and
restoring its verified pre-update snapshot, followed by recovery reconciliation. Rehearse that
procedure before relying on it.

## Accept for clinical use

On **Installed versions**, **Running version** is detected automatically. A person with
configuration-approval authority can choose **Accept running version** directly, or
**Accept this version for clinical use** in its details. Enter a note describing the checks
and confirm your identity. This task requires neither a typed version nor an update package
or a backup selection. The action appears while the running version awaits acceptance.

Acceptance records a laboratory decision; it is not certification or software validation. The
laboratory director or designee remains responsible for the actual clinical-use decision. The
release contents, support window and expedited security-update process remain open under
[Q-34](../planning/open-questions.md#decision-and-discovery-queue).

## “Started without approval”

The first version used to create an installation is the bootstrap exception. For every later
version, **Started without approval** means that no current, nonwithdrawn approval existed before
that version's first start. This usually means a package was installed outside the recorded
workflow or the wrong package or version was used.

The fact remains in the record if approval is added afterward; later approval does not rewrite the
earlier start. Record the approval, restart that version under the approval, then accept it. Health
continues to show an alert and version work item until the version is accepted.

An unapproved or unaccepted version does not block clinical actions by the current conservative
default. It shows a banner and work item instead. That choice remains laboratory question M4-L9 in
[laboratory recovery policy inputs](../quality/laboratory-policy-inputs.md#operations). The laboratory should stop,
investigate the package and approval evidence, and decide whether recovery or documented acceptance
is required before clinical use.

**Dismiss notification** hides the app-version banner for the current signed-in view. The
health indicator, version record and work item retain their state. The banner returns after
signing in again or restarting, when its version or warning changes, or when a cleared warning
recurs. Dismissal records no approval or acceptance. The recovery-reconciliation banner remains
visible while its gate applies.
