# Backups and the recovery kit

Backups are encrypted copies written to folders chosen by the laboratory. They run locally and do
not need a cloud service. The live database always remains in the computer's local data folder;
only backup copies may go to another disk or a network share.

The built-in `vtbk1` format and target layout are documented in
[Backup and export formats](../architecture/backup-and-export-formats.md). The current implementation does
not include a lab-owned object bucket, a managed cloud vault or hosted monitoring. Those are
deferred in the [backup contract](../architecture/operations-contract.md#backup-contract).

## Choose targets

Open **Backups → Targets** and choose **Add target**. Enter a PHI-free **Name**, choose the
**Kind** (Second internal disk, Removable drive or Network share) and what the target
**Receives** (Backups and audit anchors, or Audit anchors only), then choose **Choose folder** and
pick the folder in the operating-system dialog. The application writes only inside a `VibeToxLIS`
folder there, starting with a small marker file. Cancelling the folder dialog adds nothing.

| Target | What it protects against | Limitation | Recommended use |
| --- | --- | --- | --- |
| Second internal disk | Failure of the primary disk | Shares the computer's theft, fire, ransomware and administrator boundary | Fast additional copy, not the only copy |
| Removable drive | Host loss and, while disconnected, ransomware | Depends on regular rotation and secure storage | Keep at least one recently written drive disconnected and away from the computer |
| Network share | Host or local-disk loss | May share the laboratory network's ransomware risk; immutability depends on the server | Off-host online copy; never put the live database here |

Only removable drives and network shares count as off-host. The current default expects a rotated
removable target and warns when no offline copy is recent; the laboratory must approve its actual
rotation under [M4-L3](../quality/laboratory-policy-inputs.md#operations).

To retire a target, open its row menu or panel, choose **Retire target** and give a reason.
Retiring stops new backups and anchors from going there. Existing copies remain and can still be
used for a restore. **Show retired** lists retired targets.

## Schedule and retention

The application backs up only while it is open. A missed scheduled backup becomes due after the
next start. A newly reconnected removable target is due immediately when it is behind. Manual and
pre-update backups run ahead of scheduled work.

These backup and recovery defaults apply until the laboratory saves its own **Operations
settings** (see [that page's help](../help/operations-settings.json) for every field and range).
Every backup is a full copy. The hourly default is a project operating choice; the laboratory
must qualify copy duration, disk I/O, free space and retained-copy size at its actual volume.
The hourly default does not meet the proposed 15-minute recovery point (NFR-04, NFR-30). A
shorter full-copy interval must be qualified for copy duration, disk I/O and wear; incremental
backups are proposed in [ADR 0014](../architecture/decisions/0014-incremental-backups.md).

- **A laboratory that saved its own settings** keeps them. To adopt changed defaults, it saves a
  new version with a reason, which is versioned and audited as usual.
- **The warning age must exceed the interval** when settings are saved, so that the backup-age
  check does not warn on every cycle.

| Setting | Current default | Source and laboratory decision |
| --- | --- | --- |
| Backup interval | 60 minutes | [backup contract](../architecture/operations-contract.md#backup-contract), laboratory question M4-L2 |
| Recent sets kept per target | 6: the last working hours | [backup contract](../architecture/operations-contract.md#backup-contract), laboratory question M4-L2 |
| Daily sets kept | Newest set per UTC day for 7 days | [backup contract](../architecture/operations-contract.md#backup-contract), laboratory question M4-L2 |
| Weekly sets kept | Newest set per ISO week for 4 weeks | [backup contract](../architecture/operations-contract.md#backup-contract), laboratory question M4-L2 |
| Manual and pre-update sets | All kept; automatic pruning does not remove them | [backup contract](../architecture/operations-contract.md#backup-contract); retention and legal-hold policy remains Q-14 in the [decision queue](../planning/open-questions.md) |
| Backup-age warning and alert | Warning after 2 hours (two missed intervals); alert after 4 hours without a verified copy of committed changes | [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle), laboratory question M4-L2 |
| Off-host-copy warning | 24 hours | [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle), laboratory question M4-L3 |
| Offline-copy warning | 7 days | [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle), laboratory question M4-L3 |
| Restore-test warning | 90 days, and alert if never run | [host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle), laboratory question M4-L4 |
| Recovery-kit storage confirmation | Every 90 days | [M4-L5](../quality/laboratory-policy-inputs.md#operations) |
| Audit seals and anchors | Every 15 minutes while the app is open, and after every backup, whatever the backup interval. This is fixed, not a setting. The anchor-lag warning stays at 30 minutes | [audit seals and anchors](../architecture/operations-contract.md#audit-seals-and-anchors), laboratory question M4-L6 |

With these defaults a target holds at most 15 automatic sets, plus every manual and pre-update
set. The newest set counts as recent, daily and weekly at once, so the maximum is 6 recent, 6 more
daily and 3 more weekly sets. Each set is a full encrypted
copy of the database. Plan target space as about 15 times the database size, plus the stored files,
which are copied once each.

These defaults are not a laboratory retention schedule. The current application does not implement record-class
expiry or legal-hold deletion. The laboratory, records owner, privacy owner and counsel still must
decide Q-14 in the [decision queue](../planning/open-questions.md).

## What “verified” means

A backup counts only after the application has completed all of these checks:

1. SQLite produced a snapshot and its integrity check passed.
2. Each backup file was encrypted, synchronized to the target and read back.
3. Ciphertext length and digest matched what was written.
4. The encrypted manifest opened with the installation key and matched the manifest held in
   memory.
5. The temporary set folder was renamed into its final place.

A verified copy shows that the copy was written consistently. It does not prove that the entire
installation can be restored. Only a restore test performs that stronger check. The three levels
are defined in [verification expectations](../architecture/operations-contract.md#verification-expectations).

## Restore tests

On **Backups → Targets**, select a target and choose **Request restore test** (the dialog is titled
**Test a restore from** the target's name). Choose a **Backup set** or leave the choice empty to test
the newest set on that target, then choose **Start restore test**. Results appear on
**Backups → Restore tests**. The application decrypts it into
a scratch folder, opens the database with the current application, verifies the database and audit
chains, compares available anchors, and verifies every stored file. Live data is not changed. The
scratch copy is removed after the test.

The result records its elapsed restore time and findings. A reported `lost_interval` can be an
expected statement that newer anchored work is absent from an older backup; any other finding
fails the test. Investigate a failure and test another trustworthy set rather than assuming the
copy is usable.

Complete a restore rehearsal before a pilot. The current software warns after 90 days without a
passed restore test, but the laboratory must set and approve its cadence under
[M4-L4](../quality/laboratory-policy-inputs.md#operations). Q-31 also
leaves the long-term verification location to the laboratory; a second lab-controlled computer is
the proposed default ([decision queue, Q-31](../planning/open-questions.md#decision-and-discovery-queue)).

## The recovery kit

The installation recovery kit restores encryption access. It is separate from each person’s
security-key recovery code and cannot replace a lost sign-in key. Keep current account recovery
codes and spare keys in protected custody as described in [account recovery](installation-and-first-start.md#manage-your-keys-and-recover-access).

Open **Recovery kit** and choose **Export recovery kit**. In the dialog, fill in **Where the
disk-encryption recovery key is kept** (describe the place; never type the recovery key itself),
confirm with your password or security key, choose **Choose where to save** and pick a save location. If no file is
chosen, nothing is saved.

The plain-text kit contains:

- the installation ID, kit number and creation details;
- the application, platform, operating-system and data-folder details;
- the installation key and its fingerprint;
- backup target locations at the time of export;
- the laboratory's reference for locating the disk-encryption recovery key;
- restore instructions.

Anyone with the kit can read the encrypted backups and can restore the key that protects TOTP
secrets. Keep it like a disk-encryption recovery key. The current custody default is two
laboratory-controlled places away from the computer, with storage confirmed in the application at
least every 90 days ([M4-L5](../quality/laboratory-policy-inputs.md#operations)).
Do not keep the only kit on a backup drive beside the computer.

Export a new kit when the application says it is outdated. A kit becomes outdated when none has
been exported, the installation key changed, or backup targets changed. After placing the newest
kit in both locations, choose **Confirm where the kit is stored**, name both locations in **Where
the kit is stored** and choose **Confirm storage**. Do not put key material in that note.

## Backup and recovery health signals

| Signal | What it means | What to do |
| --- | --- | --- |
| Backup target | No active target exists | Add at least an off-host target and preferably both online and rotated-offline copies |
| Backup age | Committed changes are older than the laboratory threshold without a verified copy | Check target availability and the backup runs, then request **Back up now** |
| Backup failures | Backup runs, or copies to one target, failed since the last success: a warning from one, an alert at three in a row | Open the backup runs, fix the cause they show, such as a full or unwritable target, then request **Back up now** |
| Off-host copy | No recent verified copy exists on a removable drive or network share | Connect an off-host target and run a backup |
| Offline copy | No recent verified copy exists on a now-disconnected removable drive | Rotate a removable drive, wait for a verified copy, then disconnect and store it |
| Restore test | No passed test exists or the last one is too old | Run a restore test and investigate every failure |
| Recovery kit | The kit is missing, outdated or its storage confirmation is old | Export the latest kit to two off-computer locations and confirm them |
| Audit anchors | Anchors are not reaching an off-host target within the threshold | Connect an off-host target and review anchor status; anchoring lag does not by itself block report release |
| Stored files | A backup found a missing or damaged stored file | Stop treating the newest copy as complete, preserve evidence and investigate the affected files |
| Database integrity | The newest backup snapshot failed its integrity check, so the database may be damaged; nothing was copied, and the next intact snapshot clears the alert | Keep the earlier backups safe and follow the incident procedure to check the database before restoring anything |
| Free disk space | The data disk may not have room for a safe snapshot | Free space or move to a larger local disk before retrying |
| Installation key | The key is missing, mismatched or the OS key store is unavailable | Use the installation-key panel and the newest recovery kit; do not create a replacement laboratory by mistake |

The thresholds, owners and broader checks are defined in
[host-health and lifecycle contract](../architecture/operations-contract.md#host-health-and-application-lifecycle).
