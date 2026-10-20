import { useState } from "react";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import {
  EXPORT_REPORT_PDF,
  type ObligationView,
  RECORD_DELIVERY_ATTEMPT,
  REDIRECT_DELIVERY_OBLIGATION,
} from "../contract";
import { ExportDialog } from "./ExportDialog";
import { ObligationDialog } from "./ObligationDialog";
import { RecordAttemptDialog } from "./RecordAttemptDialog";

type ObligationAction =
  | typeof RECORD_DELIVERY_ATTEMPT
  | typeof REDIRECT_DELIVERY_OBLIGATION
  | typeof EXPORT_REPORT_PDF;

/** The actions on one obligation, in the order offered, by the engine's command names. */
const ACTIONS: readonly OfferedAction<ObligationAction>[] = [
  { id: RECORD_DELIVERY_ATTEMPT, label: "Record attempt" },
  { id: REDIRECT_DELIVERY_OBLIGATION, label: "Redirect" },
  { id: EXPORT_REPORT_PDF, label: "Export PDF" },
];

/** How an obligation is named: a report goes to one destination per obligation. */
export function obligationName(obligation: ObligationView): string {
  return `${obligation.reportNumber} to ${obligation.destination.label}`;
}

interface Chosen {
  action: ObligationAction;
  obligation: ObligationView;
}

/**
 * The actions the engine allows on an obligation (REQ-RPT-003, REQ-RPT-011), the same in its
 * panel and its row menu: record an attested attempt, redirect to another verified destination,
 * or export the issued PDF. An export is never a delivery. `dialog` is the chosen action's
 * dialog, rendered by the screen.
 */
export function useObligationActions({
  onRecorded,
}: {
  /** Given the obligation as the engine returns it after an attempt is recorded. */
  onRecorded?: (obligation: ObligationView) => void;
} = {}) {
  const [chosen, setChosen] = useState<Chosen | null>(null);
  const items = (obligation: ObligationView) =>
    allowedActionItems(ACTIONS, obligation.allowedActions, (action) =>
      setChosen({ action, obligation }),
    );
  const rowActions = allowedRowActions(ACTIONS, {
    label: obligationName,
    allowed: (obligation) => obligation.allowedActions,
    onAction: (action, obligation) => setChosen({ action, obligation }),
  });
  const dialog = chosen && (
    <ActionDialog chosen={chosen} onRecorded={onRecorded} onClose={() => setChosen(null)} />
  );
  return { items, rowActions, dialog };
}

interface ActionDialogProps {
  chosen: Chosen;
  onRecorded: ((obligation: ObligationView) => void) | undefined;
  onClose: () => void;
}

function ActionDialog({ chosen, onRecorded, onClose }: ActionDialogProps) {
  const { action, obligation } = chosen;
  switch (action) {
    case RECORD_DELIVERY_ATTEMPT:
      return (
        <RecordAttemptDialog obligation={obligation} onRecorded={onRecorded} onClose={onClose} />
      );
    case REDIRECT_DELIVERY_OBLIGATION:
      return (
        <ObligationDialog
          mode={{
            kind: "redirect",
            obligationId: obligation.obligationId,
            clientId: obligation.clientId,
          }}
          onClose={onClose}
        />
      );
    case EXPORT_REPORT_PDF:
      return (
        <ExportDialog
          revisionId={obligation.revisionId}
          reportNumber={obligation.reportNumber}
          obligation={obligation}
          onClose={onClose}
        />
      );
  }
}
