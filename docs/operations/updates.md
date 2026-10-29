# Updates

The application records approval and acceptance of every update. In builds configured for
in-app updates, it also checks, downloads, verifies and installs newer published builds on
**Versions and updates → Available updates**
([ADR 0020](../architecture/decisions/0020-in-app-updates.md)). The project's
releases are development builds without OS code signing, for evaluation with synthetic data
([branches and releases](../development/releases.md)). Installing one from the app does not
make it safe for clinical use.

## Available updates

**Available updates** asks the project's GitHub releases when the tab opens or when you choose
**Refresh**, which spins while it checks. It never checks on a schedule.

- **Versions and updates** opens on **Available updates**, before **Installed versions** and
  **Update approvals**.
- The status box at the top says **This app is up to date**, **An update is available** or
  **Updates could not be checked**, with the installed version, when the app last checked and
  **Refresh**. Up to date means no newer published version; it does not mean accepted for
  clinical use.
- Below it, **Available versions** has a **Nightly / Release** switch while a nightly build
  runs; nightly is chosen first. An official release installation shows no switch.
- It shows only the newest version of the chosen channel, which includes every earlier one, with
  **Update to version N** at its bottom right when it is newer. A nightly build also shows its
  build date, commit and the first three lines of **What's new** (**Show all N changes** shows
  the rest); these list the changes since the previous nightly build only. An official release
  shows its version, status and release date. When the installed version is the
  newest, it is marked **Running** and the status box says **This app is up to date**.
- Only a version newer than the running one can be installed. An older build cannot open a
  database that a newer version has already migrated.
- A build without the project's update key cannot update itself and says so. This applies
  to local developer builds and to builds published before in-app updates. Install a newer
  published release by hand once.

To install a newer build with configuration and backup authority:

1. Choose **Update to version N**. If the installation has no backup folder yet, the dialog
   says so and offers **Add backup folder**.
2. Confirm that you reviewed the changes. The risk class and approval notes are prefilled;
   change them if the publisher gave a risk class or other checks were made. Confirm your
   identity and choose **Update now**.
3. The app then runs each step, shown in the dialog:
   - downloads the package for this computer's system and checks its Minisign signature and
     signed version (a package already downloaded is checked again instead);
   - takes a pre-update backup and waits until its copy is verified;
   - records your approval of exactly this package with that backup;
   - refuses to install while listeners or other work are in progress; the engine checks the
     approval and the backup again and audits the installation.

   A failed step stops the update with its reason; the dialog stays open to try again.
4. The app closes and starts the new version.
   - **Windows:** the installer shows its progress.
   - **Linux:** a `.deb` installation asks for the computer's administrator password; an
     AppImage is replaced in place.
   - **macOS:** installation from the app is not yet qualified.
5. The new version awaits acceptance for clinical use.
6. Continue with [Install and start](#install-and-start) step 4 and
   [Accept for clinical use](#accept-for-clinical-use).

If the download or installation fails, the running version is unchanged. A failed
installation is recorded in the audit trail. A downloaded package is discarded when the app
restarts.

## Before approving

1. Obtain the release package through the laboratory's approved route. Review the change summary,
   published risk class, known limitations, migration notes and available verification evidence.
2. If appropriate, restore a recent backup onto an isolated test computer. Never close the test
   copy's recovery reconciliation or connect it to production targets, analyzers, destinations or
   printers.
3. Check **Backups → Runs** for a recent verified manual or pre-update backup. If needed,
   choose **Request pre-update backup** inside the approval dialog.
4. Wait for that run to show a verified copy. A queued or running backup is not enough.

The application accepts a manual or pre-update backup only when it completed no more than
24 hours before approval and still has a verified, unpruned copy. Scheduled backups do not
qualify because automatic retention may remove them. These are application design rules in
[update-record contract](../architecture/operations-contract.md#update-records).

## Record approval

An administrator with configuration authority chooses **Approve a package from outside the
app** on **Versions and updates → Update approvals**. **Record update approval** asks for:

- **Version to install**, copied from the downloaded update's release notes (package version
  detection is not implemented);
- the published risk class;
- confirmation that the release notes were reviewed;
- **Backup before this update**, selected by completion time and verified destination;
- approval notes;
- identity confirmation using the account's sign-in method.

For a security-key account, both **Record update approval** and **Update now** on
**Available updates** use the selected registered key. Submit the form, then enter that key's
PIN or complete its supported biometric verification when prompted. Canceling or failing
verification records no approval and starts no installation. The optional six-digit LIS session-unlock
PIN cannot confirm an update.

The backup choices come from the installation's recorded **Backups → Runs** history, not a
scan of files on disk. **Load older backup runs** continues past the initial 100 runs;
**Refresh backup list** checks for newly completed work. The engine determines eligibility
and rechecks it when saving, so a backup that expires while the form is open may be refused.

For a package obtained outside the app, the application then opens a file dialog for the update
package, computes its SHA-256 digest and records that package with the approval. This step does
not verify a publisher signature and does not install the package. For a package downloaded on
**Available updates**, the downloaded file is recorded instead and the app installs it.

## Install and start

1. For a package obtained outside the app, close the application safely. Do not override
   listener or pending-work warnings merely to start the update.
2. Install the package using the controlled procedure supplied with that release, or let
   **Available updates** install a downloaded package.
3. Open the application. Before applying pending database migrations, it makes an integrity-checked
   local safety snapshot when required. If that copy fails, startup stops rather than migrating.
4. Review startup, host health, listener state, the web-engine change check and the laboratory's
   release-specific smoke tests.

Automated rollback is not built. The planned rollback is reinstalling the prior package and
restoring its verified pre-update snapshot, followed by recovery reconciliation. Rehearse that
procedure before relying on it.

## Accept for clinical use

On **Installed versions**, **Running** marks the version that runs now, with its status. A person
with configuration-approval authority chooses **Accept this version for clinical use** in its row
menu or its details. Enter a note describing the checks and confirm your identity. This task requires neither a typed version nor an update package
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
