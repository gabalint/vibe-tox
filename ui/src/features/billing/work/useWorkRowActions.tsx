import { useState } from "react";
import { Navigate } from "react-router";
import type { BillingWorkItem } from "@/generated/BillingWorkItem";
import { allowedRowActions } from "@/lib/allowedActions";
import { DeriveDialog } from "../DeriveDialog";
import { RecordOutcomeDialog } from "../handoffs/RecordOutcomeDialog";

/** Deriving the services of work not yet derived: that row's usual next step. */
export const DERIVE_SERVICES = "derive_services";

/** The row actions on one entry, by the engine's command names, in the order offered. */
const ACTIONS = [
  { id: DERIVE_SERVICES, label: "Derive services" },
  { id: "record_handoff_outcome", label: "Record outcome" },
  { id: "record_handoff_followup", label: "Review rejected handoff" },
];

/** How a row is named: its accessions, as the list shows them. */
export const accessionsText = (row: BillingWorkItem) => row.accessionNumbers.join(", ");

interface Chosen {
  action: string;
  row: BillingWorkItem;
}

/**
 * The actions on one billing work entry that its row menu offers, from the row's
 * `allowedActions` (REQ-BIL-002, REQ-BIL-005): derive its services, or record the billing
 * partner's outcome of its handed-off item. Approval needs the derivation's revision, so it
 * stays on the derivation's page; handing off acts on several rows, through the selection.
 */
export function useWorkRowActions() {
  const [chosen, setChosen] = useState<Chosen | null>(null);
  const rowActions = allowedRowActions(ACTIONS, {
    label: accessionsText,
    allowed: (row) => row.allowedActions,
    onAction: (action, row) => setChosen({ action, row }),
  });
  const close = () => setChosen(null);
  return {
    rowActions,
    /** Starts an action on a row; the engine still checks it. */
    start: (action: string, row: BillingWorkItem) => setChosen({ action, row }),
    dialog: chosen && <ChosenDialog chosen={chosen} onClose={close} />,
  };
}

function ChosenDialog({
  chosen: { action, row },
  onClose,
}: {
  chosen: Chosen;
  onClose: () => void;
}) {
  if (action === DERIVE_SERVICES) {
    return (
      <DeriveDialog
        input={{ subjectId: row.subjectId, dateOfService: row.dateOfService }}
        onClose={onClose}
      />
    );
  }
  // The engine offers an outcome only for a handed-off item, which names its ID.
  if (action === "record_handoff_followup" && row.handoffId !== null)
    return <Navigate to={`/billing/handoffs/${row.handoffId}?open=${row.itemId}`} />;
  return row.itemId === null ? null : <RecordOutcomeDialog itemId={row.itemId} onClose={onClose} />;
}
