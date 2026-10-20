import { Alert } from "@/components/Alert";
import { TextInput } from "@/components/FormFields";
import type { ExportedKit } from "@/generated/ExportedKit";
import type { ExportRecoveryKitInput } from "@/generated/ExportRecoveryKitInput";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { EXPORT_RECOVERY_KIT, KIT_QUERIES } from "../contract";

/**
 * Exports the recovery kit after the person signs again (design brief §2.4 and §3.3). The app
 * opens the save dialog first; the engine then checks the password, records the export and
 * writes the file. A wrong password shows on its field and nothing is written.
 */
export function ExportKitDialog({ onClose }: { onClose: () => void }) {
  return (
    <ReauthConfirmDialog<{ diskEncryptionReference: string }, ExportRecoveryKitInput, ExportedKit>
      title="Export recovery kit"
      confirmLabel="Choose where to save"
      variant="primary"
      command={{ name: EXPORT_RECOVERY_KIT, invalidates: KIT_QUERIES }}
      extraDefaults={{ diskEncryptionReference: "" }}
      toInput={({ diskEncryptionReference, password }) => ({
        diskEncryptionReference: diskEncryptionReference.trim(),
        password,
      })}
      successTitle={(kit) => `Recovery kit ${kit.kitNo} saved as ${kit.fileName}`}
      onClose={onClose}
      errorTitle="Not exported"
      cancelledText="No file was chosen, so the recovery kit was not saved."
    >
      <Alert tone="warning">
        Anyone with this file can read your backups. Store it like a disk-encryption recovery key,
        in two places off this computer.
      </Alert>
      <p>
        The kit holds the installation key, the backup locations and the restore steps. After you
        confirm, choose where to save it.
      </p>
      <TextInput
        name="diskEncryptionReference"
        label="Where the disk-encryption recovery key is kept"
        description="Printed in the kit, so whoever restores can find it. Do not type the recovery key itself."
        isRequired
      />
    </ReauthConfirmDialog>
  );
}
