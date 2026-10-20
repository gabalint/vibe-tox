import { Alert } from "@/components/Alert";
import type { ExitExportView } from "@/generated/ExitExportView";
import type { RequestExitExportInput } from "@/generated/RequestExitExportInput";
import { ReauthConfirmDialog } from "@/lib/ReauthConfirmDialog";
import { EXIT_EXPORT_QUERIES, REQUEST_EXIT_EXPORT } from "../contract";

/**
 * Requests the exit export (REQ-OPS-010, NFR-12; design brief §5): every record, in an open
 * format, in a folder the person chooses next. It is the most sensitive command (E15), so it
 * needs the password and a reason, and it is audited.
 */
export function ExitExportDialog({ onClose }: { onClose: () => void }) {
  return (
    <ReauthConfirmDialog<Record<string, never>, RequestExitExportInput, ExitExportView>
      title="Export all data"
      confirmLabel="Choose the folder"
      variant="danger"
      command={{ name: REQUEST_EXIT_EXPORT, invalidates: EXIT_EXPORT_QUERIES }}
      extraDefaults={{}}
      withReason
      toInput={({ reason, password }) => ({ reason: reason.trim(), password })}
      successTitle={(exported) => `Exit export requested to ${exported.folderLabel}`}
      onClose={onClose}
      errorTitle="Not exported"
      cancelledText="No folder was chosen, so nothing was exported."
    >
      <Alert tone="warning" title="The package is not encrypted">
        It holds patient information in plain, readable files. Save it only to encrypted media, and
        keep it as you keep patient records.
      </Alert>
      <p>
        The package holds the database, every stored file, the audit anchors, a data dictionary and
        an independent verifier, so the data can be read and checked without this app. After you
        confirm, choose the folder to write it to.
      </p>
    </ReauthConfirmDialog>
  );
}
