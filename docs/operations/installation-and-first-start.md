# Installation and first start

Status: operator instructions for the current application. Signed
installers are not available yet. The publisher, signing route and release channel remain open
under [Q-19 and Q-34](../planning/open-questions.md#decision-and-discovery-queue). Do not treat a
developer build as an approved clinical release.

## Prepare the laboratory computer

The proposed supported-platform baseline and its reasons are in the
[deployment host baseline](../architecture/deployment.md#host-baseline), with its requirement in
[NFR-25](../requirements/nonfunctional-requirements.md). A release must name
the exact operating-system and web-engine versions it was tested with. Real-host verification of
the encryption, OS, power, dialog, restore and shutdown checks is not complete on any platform.

Before first start:

1. Turn on full-disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux.
2. Put the disk-encryption recovery key under laboratory control and away from the computer. Do
   not keep the only copy in a personal cloud account.
3. Use an SSD and a UPS. Run the application as a standard, non-administrator operating-system
   user.
4. Set the operating system to remain signed in while analyzers run. The display may lock.
5. Keep the computer clock synchronized.

These are current baseline expectations, not proof that a particular host is suitable. The
laboratory and technical maintainer must confirm the actual hardware under [Q-29](../planning/open-questions.md#decision-and-discovery-queue).

## The data folder

The application chooses its per-machine local application-data folder. The exact path appears on
the **Host health** page and on the startup-failure screen when startup fails.

The live data folder must be on a disk attached to this computer. Do not place it on a NAS, network
share, roaming profile, synchronized folder or mounted remote file system. The application refuses
to start when it detects a network data folder; on an unusual drive that it cannot classify, it may
also refuse rather than risk opening SQLite on unsafe storage. Network shares are allowed only as
backup targets. This rule is defined in the [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).

## Choose a demo or an empty laboratory

On **Set up this laboratory**, **Explore the demo laboratory** opens an evaluation workspace
with its own data folder. **Start an empty laboratory** creates the first account in the
laboratory's own installation. Choose the demo before entering account details: opening it
restarts the app and does not save values entered on the original setup screen. This choice is
available before the laboratory's first account is created.

If replacing an existing installation, use **Restore from a backup** instead of creating a new
account. Follow [Restore and recovery](restore-and-recovery.md).
The restore action appears below the account form. A pending restore or failed restore is shown
above it: **A restore is ready to finish** requires closing and reopening the app; **The last
restore could not be applied** explains why the current data was kept. Resolve those notices
through the recovery guide before creating an unrelated new account.

### Explore with the bundled dataset

1. Choose **Explore the demo laboratory**, then confirm **Open the demo laboratory**. The app
   restarts into the separate demo. **Cancel** keeps the current setup screen.
2. On **Set up your demo laboratory**, enter the account fields below. Leave **Load Standard
   Catalog and demo data** checked to prepare the whole bundled dataset automatically.
3. Choose a **Sign-in method**, then **Create demo account**. For **Security key**, verify your
   personal key and save its recovery code. For **Password + authenticator app**, sign in with
   your chosen password, enroll an authenticator app and enter its current code. There is no
   shared demo credential or sign-in shortcut.
4. Wait for **Preparing your demo laboratory**. The app loads the Standard Catalog as drafts,
   the fictitious laboratory profile, 4 clinics with their locations and delivery destinations,
   8 practitioners and 12 patients. It opens the workspace after preparation completes.
5. Explore **Patients**, **Clients** and the catalog. Before trying an order, go to
   **Laboratory setup › Catalog review**, generate a preview, inspect it and approve the
   reviewed drafts. Catalog loading does not perform this approval.

| Setup field | Meaning, default and limits |
| --- | --- |
| Display name | Required individual name shown in the app and audit trail; trimmed, 1–100 characters, with no control characters. |
| Username | Required sign-in name; 3–64 ASCII letters, digits, periods, hyphens or underscores, starting with a letter. It is trimmed and stored lowercase. |
| Sign-in method | **Password + authenticator app** is the default. **Security key** instead registers a personal FIDO2 key and requires its PIN or supported biometric verification; no separate LIS password or authenticator app is required for that account. |
| Password / Confirm password | Password route only. Required personal password, 12–128 characters; must not contain the username, regardless of case. Enter it again to confirm. |
| Key name | Key route only. Required descriptive name, trimmed, 1–80 characters without control characters. For example, **Everyday key**. This names the key in your account, not the person or their role. |
| Load Standard Catalog and demo data | Demo setup only; checked by default. Loads the complete shipped catalog and synthetic dataset after successful authentication. This option gives the demo account every role and allows one-person work; self-approval still asks for confirmation. |
| Who will use this laboratory? | Shown for an empty laboratory or when demo loading is unchecked. **Several people** is the default and creates an Administrator account. **Only me: I run this laboratory alone** grants every role and permits one-person approvals under the separation policy. An administrator can change that policy later. |

Unchecking the dataset option creates an empty demo. You can load selected catalog content later
through **Laboratory setup › Starter data**, followed by **Load Demo Data** while signed in as
the same person. That manual load needs Administrator and Accessioner authority together.

The prepared demo contains no additional staff accounts, configured analyzers, lots, orders or
results. Its catalog stays in draft until reviewed. Some patient examples deliberately open
identity holds, and supported blank draft cutoffs receive explicitly demo-only values. Use the
bundled **Starter data and catalog review** help topic for the field-by-field load and review
instructions. Demo data and reports remain marked for evaluation.

### Retry, leave or reset the demo

| Situation or action | Result and recovery |
| --- | --- |
| Checking the laboratory… / The laboratory could not be identified | Account creation waits while the app identifies the installation. If the check fails, choose **Try again** to read its status; setup cannot continue until the laboratory or demo is identified. |
| Account fields are rejected | Correct the highlighted fields and submit again. A first account can be created only while no account exists in that installation. |
| The demo could not be prepared | Read the error and support reference. **Try again** retries preparation without duplicating the dataset; account creation has already succeeded. Preparation either commits the catalog and demo records together or leaves the load pending. A permissions refusal requires the original setup account with the required roles, rather than repeated retries. |
| The error asks for the account created during demo setup | Choose **Sign out**, then sign in with the original setup account. Only that account can complete its pending preparation request. If sign-out fails, the error shows its own support reference and the action remains available. |
| The app closes before preparation finishes | Open it again, sign in to the same demo account and complete authentication. The saved dataset choice resumes preparation. |
| The setup status could not be checked | Use **Try again** to reread status; the app waits for a successful check before opening the workspace. |
| Leave the demo laboratory | Confirm **Leave the demo laboratory** to restart in the laboratory's own installation. The demo is retained. **Cancel** keeps the current demo. |
| Reset the demo laboratory | Confirm **Delete the demo data** to remove all demo accounts, data and work entered there, then restart with an empty demo. The laboratory's own data is retained. **Cancel** preserves the demo. |

Leaving and resetting are available from the permanent demo banner, including before sign-in.
Switching can be refused while application work is still running; follow the reported reason
and wait for that work to finish. A demo installation cannot become a real installation, and
its backup cannot be restored into a real laboratory.

## First start for a new laboratory

If this computer replaces an earlier installation, stop here and use [Restore and recovery](restore-and-recovery.md).

1. Open the application. Under **Start an empty laboratory**, enter the first administrator's
   display name, username and **Sign-in method** using the field rules above. Supply either a
   key name or password and confirmation. Choose who
   will use the laboratory, then **Create administrator**. The default **Several people**
   grants the Administrator role only. The one-person choice grants all roles and permits
   one-person work as described in the [operator guide](README.md#separation-of-duties-and-one-person-laboratories).
2. Complete the selected route below. The security-key route opens the session after you confirm
   saving its recovery code. The password route asks you to sign in and enroll an authenticator app.
3. Open **Users** and create a separate account for each person. Each new person receives a
   temporary password, chooses a new password at first sign-in and enrolls an authenticator app.
   They can then switch their own account through **Signed in as … → Security keys**. Direct
   key registration for additional staff at account creation is not yet offered.
4. If setup used **Several people**, appoint the first quality manager. The first administrator
   may do this only while no active quality manager exists. Afterward, clinical roles require quality authority. See
   [Q-37](../planning/open-questions.md#decision-and-discovery-queue).
5. Decide and document whether one person may hold both administrator and quality-manager roles.
   The software permits it, but the residual separation risk belongs to the laboratory
   ([Q-37](../planning/open-questions.md#decision-and-discovery-queue)).
6. Open **Laboratory profile** and enter the laboratory name, address, CLIA number, laboratory
   director, phone and IANA time-zone name. Billing NPI and tax ID are optional fields. Reports
   cannot be prepared until the profile exists; screens show UTC until its time zone is set.
7. Configure users, instruments, printers and laboratory-owned policies only from approved
   information. Do not invent assay rules, cutoffs or interpretations.
8. Before relying on the computer, add backup targets, export the recovery kit and complete a
   restore test as described in [Backups and recovery kit](backups-and-recovery-kit.md).

## Sign in with a personal security key

By default, use a FIDO2 key that supports user verification with a configured PIN or supported
biometric. First-account setup and recovery always require this verification. Configure its PIN
with the key manufacturer's management software before registration when needed; the LIS
does not create or change a key PIN. An authorized administrator can allow compatible keys without
a configured PIN for later enrollment and sign-in, as described below.
Hardware and operating-system qualification is still pending;
a successful synthetic test does not establish support for a particular key. Plain application
PINs for initial sign-in, Windows Hello and synced passkeys are not offered. The optional
[personal unlock PIN](#optional-personal-unlock-pin) below is a separate session convenience.

On macOS and Linux connect exactly one USB security key before starting. Windows uses its native
security dialog. Select an external security key there. Keep the computer offline if your operating
procedure requires it; this flow does not contact an identity provider.

| Control or step | Meaning and outcome |
| --- | --- |
| First setup: Create administrator / Create demo account | Verify the key and wait for **Save your recovery code**. Account creation commits only after successful registration. Cancellation or a failed registration leaves setup available. |
| Security key PIN / Continue | Enter the PIN belonging to the physical key when requested. It is not an LIS password. The key checks it; key-specific retry limits apply. Windows may collect it in its own dialog. The app does not impose a universal PIN length or guarantee a new PIN entry for every assertion. |
| Touch prompt / Checking the security key… | Touch the connected key when it signals, then wait for verification. Touch proves physical presence. First-account setup, recovery and sensitive actions also require the key's PIN or biometric verification; later sign-in follows the laboratory policy and the protection retained for that key. |
| Save your recovery code | Copy the shown 64-character code to secure custody away from this computer. It is displayed once. Check **I saved this code away from this computer**, then **Continue**. Initial signup opens your session only after this acknowledgment. |
| Continue to sign in after confirmation fails | The account and key already exist. Keep the displayed recovery code, confirm it is saved, then choose **Continue to sign in** and **Use a security key**. This does not retry the expired setup confirmation or create another account. |
| Later sign-in: Use a security key | Enter **Username**, then **Find my keys**. Select one named **Security key**; the first returned key is selected initially. Choose **Sign in with security key** and complete verification. **Use another username** returns to account selection. |
| Use password and authenticator app | Returns to the separate password route. It does not enable password fallback for an account switched to security keys. |
| Cancel / Escape during a key prompt | Cancels that operation. On Windows, also cancel the operating-system security dialog; the app cannot dismiss it itself. Hardware operations and the spare-key handoff have bounded waits. A timeout requires a new deliberate attempt. |

The initial session confirmation expires after 15 minutes. If it fails, save the still-visible
recovery code and use **Continue to sign in**. An interrupted first setup after registration can
also leave a registered account without an open session; reopen the app and sign in using that
key. If the recovery code was not saved, add a spare key after signing in to issue new recovery
material. Do not create a shared substitute account.

### Allow security keys without a PIN for sign-in

Under **Operations settings → Security policy → Change security policy**, an administrator with
**Manage users** can check **Allow security keys without a PIN for sign-in**. It is unchecked
by default. Approve this exception through the laboratory's own workstation and access policy:
someone possessing a lost or unattended touch-only key may be able to use its owner's account.
Keep each key in its owner's custody and remove it when leaving the workstation.

Changing this checkbox in either direction requires fresh confirmation with the administrator's
password or a registered key's PIN/biometric verification. Enabling it needs a reason even for the
first saved policy; every later policy version also needs a reason. Choose **Save settings** and
check the displayed **Security key sign-in** value. All other laboratory sessions end; the
administrator's current session stays open. **Cancel** leaves the current policy unchanged.

| Policy or key state | Behavior |
| --- | --- |
| Option off, the default | New enrollments and key sign-in/unlock require PIN or biometric verification. |
| Option on | A compatible key without configured verification can be registered through your signed-in **Security keys** dialog for touch-only sign-in/unlock. A configured PIN or biometric check is still used when the key requests it. |
| Key previously enrolled with verification | Retains that protection when the laboratory enables touch-only sign-in. The option does not bypass a wrong or blocked key PIN. |
| Sensitive action, including adding/revoking keys or changing the personal unlock PIN | Still requires PIN or biometric verification. Configure it on your key using its management software, or select an existing registered key that supports it. A touch-only sign-in alone cannot confirm the action. |
| First-account setup or saved-code recovery | Always requires a new/replacement key with PIN or biometric verification. The policy exception does not apply. |
| Option turned off later | Previously enrolled touch-only keys now need PIN or biometric verification for sign-in/unlock. Arrange that capability or another verified key before disabling the exception. Personal unlock-PIN settings are unchanged. |

If confirmation is cancelled, fails or the policy version changes meanwhile, the change is not
saved. Reopen the policy, review the current values and submit a new confirmation. Do not assume
the checkbox took effect before the success message.

## Manage your keys and recover access

**Signed in as … → Security keys** manages your own credentials. **Switch to a security key**
requires your current password and a new key registered under the laboratory's policy above.
The enrollment form shows that policy and any touch-only warning. Successful migration removes
password/TOTP sign-in for that account. **Add security key** verifies an existing key using its
PIN or biometric check first; when prompted,
disconnect it, connect a different physical key and choose **Continue with the new key**. Registering
the same credential twice is refused. Store the spare separately.

Every successful enrollment issues a new recovery code and invalidates the previous one. Save the
new code before dismissing the confirmation. **Revoke [key name]** asks for a reason and fresh key
verification, then revokes that key and ends all of your sessions, including the current one. The
last active key cannot be revoked; enroll a replacement first. Your roles and clinical permissions
do not change when managing credentials.

If every registered key is unavailable, choose **Use a security key → Recover with a saved code**
on the sign-in screen. Enter **Username**, the current **Recovery code**, and a **Key name**, then
choose **Register replacement key**. The code authorizes replacement only. Registration requires
the replacement key's PIN or biometric verification, even if touch-only sign-in is allowed.
Successful registration revokes the old keys, consumes the old code, ends account sessions and
displays a new recovery code.
Save it, choose **Continue**, and sign in with the replacement key. A cancelled or failed replacement
does not consume the code. **Back to sign in** leaves recovery before replacement starts.

If the code is invalid, check the exact latest copy: adding any key replaced earlier codes. Failed
authentication can temporarily lock the username; wait for the stated lockout. A disabled account
cannot recover until appropriately authorized administration restores its active status. If no key
or current recovery code remains, this build has no administrative key-account recovery bypass.
Follow the laboratory's downtime procedure and preserve the installation; the installation recovery
kit is not an account recovery code.

## Password and authenticator alternative

This route retains the 12–128-character password policy. After initial account creation, sign in
with that password, scan the authenticator QR code or enter its shown setup key, then enter the
current TOTP code. Later sign-ins use password and TOTP; normal same-user unlock and sensitive
confirmations use the current password. Both routes work without internet. The full phishing-resistant target
in [NFR-19](../requirements/nonfunctional-requirements.md) is not met merely by keeping this alternative.

If a sign-in step expires after credentials, roles or active status change, restart with the current
credentials. After an authorized password-account reset, use the temporary password and choose a
personal password before completing MFA. Key accounts cannot be reset to a temporary password by
this action. There is no local break-glass sign-in in this build.

The installation key that protects authenticator secrets and personal unlock-PIN verifiers is kept
in the operating system's key store. Export the installation recovery kit for a replacement computer, including for a laboratory
whose people use security keys. See [Backups and recovery kit](backups-and-recovery-kit.md#the-recovery-kit).

## Optional personal unlock PIN

After normal sign-in, any user can open **Signed in as … → Unlock PIN**. The personal PIN is
disabled by default. It resumes only that person's existing locked session and is separate from
a physical security key's PIN. Follow the laboratory's approved workstation and session policy.

1. Choose **Enable PIN** or, if already enabled, **Change PIN**.
2. Enter **New unlock PIN** and **Confirm unlock PIN**. Both are required, masked and initially
   blank. Use exactly six digits from 0–9; leading zeros are preserved. The confirmation must match.
   Identical digits, ascending/descending sequences including wraparound, repeated two- or
   three-digit blocks and common patterns (`112233`, `123321`, `159753`, `147258`, `258369`,
   `102030`) are rejected. Do not share your PIN.
3. Confirm with **Your password** or your registered **Security key**, then submit. The previous
   unlock PIN cannot confirm a change. A success message confirms saving; reopen **Unlock PIN**
   to check **Enabled**. Saving ends your other sessions and keeps the current session open.

The lock screen offers **Unlock PIN** first when enabled. Enter it and choose **Unlock**. You can
instead choose **Use password instead** or **Use security key instead**, and **Use unlock PIN**
returns to the PIN field. Switching methods clears the field. **Sign in as a different user** ends
the current session and opens normal sign-in. If PIN status cannot be checked, normal unlock and
different-user sign-in remain available.

| Situation or control | Behavior and next step |
| --- | --- |
| Incorrect PIN | The field clears and the remaining attempts update. Five incorrect submitted attempts end the current session; sign in normally again. A successful PIN unlock or confirmed PIN setting change resets the budget. Normal password/key unlock does not reset failed PIN attempts. |
| Forgotten PIN | Use your normal unlock method, then change or disable the PIN using your password or key. The old PIN is not required and cannot be displayed. |
| Restart, sign-out or 12-hour session limit | Normal sign-in is required. A PIN cannot start a new session or extend the absolute 12-hour session limit. |
| Sensitive action | Confirm with the account's normal password or registered key. The personal unlock PIN cannot approve the action. |
| Disable PIN | Open **Unlock PIN → Disable PIN**, then confirm with your password or key. A success message confirms removal. Other sessions end; the current one stays open. Future unlocks use the normal method. |
| Close / Cancel | Leaves the status dialog or abandons an unsent form. Form cancellation is unavailable while its command is pending. Unsubmitted changes are not saved. |
| Status error / Try again | Checks the current setting again. Configuration controls appear only after a successful status read. |
| Configuration cancelled or expired | A session lock, sign-out, user change, expiry, timeout or clock change can invalidate confirmation; a cancelled change is not saved. Unlock or sign in again, reopen the setting, check its status and retry with fresh confirmation. |
| Administrative credential reset or saved-code key recovery | Disables the personal PIN. Complete normal sign-in and enable a new one if wanted. |
