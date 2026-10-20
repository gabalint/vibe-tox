import { ChoiceInput } from "@/components/FormFields";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import {
  DELIVERY_QUERIES,
  EXPORT_REPORT_PDF,
  type ExportedReportFile,
  type ExportPurpose,
  type ExportReportPdfInput,
  GET_REPORT,
  type ObligationView,
} from "../contract";
import { EXPORT_PURPOSES } from "../labels";

interface ExportDialogProps {
  revisionId: string;
  reportNumber: string;
  /** The obligation the export is for, when exporting to deliver it. */
  obligation?: ObligationView;
  onClose: () => void;
}

/**
 * Saves the issued PDF, byte for byte as released, to the Downloads folder (NFR-07). An
 * export is recorded in the audit trail and is not a delivery: delivery needs an attempt
 * recorded on its obligation (REQ-RPT-011).
 */
export function ExportDialog({ revisionId, reportNumber, obligation, onClose }: ExportDialogProps) {
  const purposes = obligation
    ? EXPORT_PURPOSES
    : EXPORT_PURPOSES.filter((each) => each.id !== "delivery");
  return (
    <CommandDialog<{ purpose: ExportPurpose | null }, ExportReportPdfInput, ExportedReportFile>
      title={`Export PDF of ${reportNumber}`}
      submitLabel="Export PDF"
      command={{ name: EXPORT_REPORT_PDF, invalidates: [GET_REPORT, ...DELIVERY_QUERIES] }}
      defaultValues={{ purpose: obligation ? "delivery" : null }}
      toInput={({ purpose }) => {
        const chosen = checked(purpose);
        return chosen === "delivery" && obligation
          ? { revisionId, purpose: chosen, obligationId: obligation.obligationId }
          : { revisionId, purpose: chosen };
      }}
      successTitle="PDF saved to your Downloads folder. An export is not a delivery."
      onClose={onClose}
    >
      <p>
        The issued file is saved to your Downloads folder exactly as released. It contains patient
        information: handle it as your laboratory's procedure requires. Exporting is recorded and is
        not a delivery.
      </p>
      <ChoiceInput name="purpose" label="Purpose" options={purposes} isRequired />
    </CommandDialog>
  );
}
