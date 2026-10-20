import { TextInput } from "@/components/FormFields";
import type { AcceptInstalledVersionInput } from "@/generated/AcceptInstalledVersionInput";
import type { InstalledVersionView } from "@/generated/InstalledVersionView";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { ACCEPT_INSTALLED_VERSION, VERSION_QUERIES } from "../contract";

interface AcceptVersionDialogProps {
  version: string;
  onClose: () => void;
}

/**
 * Accepts the running version for clinical use (REQ-OPS-006, design brief §7). The engine
 * checks that it runs, was approved (or is the first version) and is not yet accepted.
 */
export function AcceptVersionDialog({ version, onClose }: AcceptVersionDialogProps) {
  return (
    <ReauthConfirmDialog<{ note: string }, AcceptInstalledVersionInput, InstalledVersionView>
      title={`Accept version ${version} for clinical use`}
      confirmLabel="Accept version"
      variant="primary"
      command={{ name: ACCEPT_INSTALLED_VERSION, invalidates: VERSION_QUERIES }}
      extraDefaults={{ note: "" }}
      toInput={({ note, password }) => ({ version, note: note.trim(), password })}
      successTitle={() => `Version ${version} accepted for clinical use`}
      onClose={onClose}
      errorTitle="Not accepted"
    >
      <p>
        You confirm that version {version}, running on this computer, has been checked as the
        laboratory requires and may be used for clinical work.
      </p>
      <TextInput
        name="note"
        label="Note"
        description="Such as the checks performed. Recorded in the audit trail."
        isRequired
      />
    </ReauthConfirmDialog>
  );
}
