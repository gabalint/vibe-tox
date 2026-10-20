# Exit export

The application can write a complete, portable folder package for leaving the application or
keeping an independently readable copy. It does not need a cloud service or a service provider.

An exit export is one of the most sensitive operations. It requires the export capability, a
reason and fresh confirmation with your password or security key, and it is recorded in the security audit stream.

## Create the package

1. Attach or prepare encrypted media with enough free space.
2. Open **Exit export** and choose **Export all data**.
3. Read the plaintext warning, enter the reason and confirm with your password or security key, and choose the destination folder.
4. Wait for the export list to show a successful terminal state. Do not use an incomplete folder.

The application creates a new folder whose name begins `vibe-tox-exit-`. A complete package has a
`manifest.json`; a matching folder without that file is interrupted and must not be used.

## Package contents

The package holds a `README.txt` with the format and verification instructions, `manifest.json`,
the integrity-checked database snapshot, every retained stored file, a line for every audit seal,
the schema, data dictionaries and migration sources, and a convenience copy of the verifier. The
exact layout is specified in
[Backup and export formats](../architecture/backup-and-export-formats.md#exit-export-vibe-tox-exit-1).

## Check the package independently

The verifier inside the package must be treated as package data until compared with a trusted
copy. If a package were altered, its embedded verifier could be altered to hide the change. First
obtain `scripts/verify_exit_export.py` from the trusted application installation or the published
source at the package's application version.

1. Compute the SHA-256 of the embedded `tools/verify_export.py` with an operating-system tool.
2. Compute the SHA-256 of the trusted verifier. Compare all 64 hexadecimal digits with each other
   and with the verifier digest printed in the package's `README.txt`.
3. Run the trusted copy from outside the package:

   ```text
   python3 -I /trusted/path/verify_exit_export.py "/path/to/vibe-tox-exit-…"
   ```

   On Windows, use the laboratory's trusted Python 3 launcher if it is named differently.

4. Accept the check only when the verifier reports every check and exits successfully. Preserve
   any failed package and its output for investigation; do not edit the package to make it pass.

Only after the embedded script's digest matches a trusted copy may it be run instead. The
anchors inside the package demonstrate only internal consistency; compare separately held off-host
anchors as well when investigating privileged tampering. The checks the verifier performs are
listed in [Checking a package](../architecture/backup-and-export-formats.md#checking-a-package).

## Media handling

The package is plaintext PHI. It is not a backup-format ciphertext bundle.

- Write it only to encrypted media controlled by the laboratory.
- Give it only to people authorized for the complete laboratory record.
- Keep the package out of ordinary email, consumer file-sharing accounts, issue trackers and
  developer support channels.
- Apply the laboratory's approved retention, legal-hold, transport and disposal procedure.
- Verify the receiving system's custody and reconciliation before treating a migration as
  complete.

The application currently exports only data held by the local installation. Optional cloud
services and export of cloud-held copies are not built.
