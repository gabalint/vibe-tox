import { type ReactNode, useState } from "react";
import type { GridRowActions } from "@/components/DataGrid";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { ActionItem } from "@/components/RowActions";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { useCommandMutation } from "@/lib/commands";
import {
  DESTINATION_QUERIES,
  type DestinationView,
  REPLACE_DELIVERY_DESTINATION,
  RETIRE_DELIVERY_DESTINATION,
  type RetireDeliveryDestinationInput,
  VERIFY_DELIVERY_DESTINATION,
} from "./contract";
import { ReplaceDestinationDialog } from "./ReplaceDestinationDialog";
import { VerifyDestinationDialog } from "./VerifyDestinationDialog";

type Command =
  | typeof VERIFY_DELIVERY_DESTINATION
  | typeof REPLACE_DELIVERY_DESTINATION
  | typeof RETIRE_DELIVERY_DESTINATION;

/** The actions on one destination, in the order the record view shows them. */
const ACTIONS: readonly OfferedAction<Command>[] = [
  { id: VERIFY_DELIVERY_DESTINATION, label: "Verify" },
  { id: REPLACE_DELIVERY_DESTINATION, label: "Replace" },
  { id: RETIRE_DELIVERY_DESTINATION, label: "Retire", variant: "danger" },
];

/**
 * The actions the engine allows on each destination (`allowedActions`), for its panel and
 * its row menu, and the one dialog they open. The dialog looks the destination up in the
 * latest list, so it always sends the current revision; the engine decides each action
 * again, such as refusing a verification by the destination's creator.
 */
export function useDestinationActions(destinations: readonly DestinationView[]): {
  actionsOf: (destination: DestinationView) => ActionItem[];
  rowActions: GridRowActions<DestinationView>;
  dialog: ReactNode;
} {
  const [pending, setPending] = useState<{ command: Command; id: string } | null>(null);
  const close = () => setPending(null);
  const destination = destinations.find((each) => each.destinationId === pending?.id);
  const start = (command: Command, target: DestinationView) =>
    setPending({ command, id: target.destinationId });
  const actionsOf = (target: DestinationView) =>
    allowedActionItems(ACTIONS, target.allowedActions, (command) => start(command, target));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: DestinationView) => row.label,
    allowed: (row) => row.allowedActions,
    onAction: start,
  });
  const dialog = pending && destination && (
    <DestinationDialog command={pending.command} destination={destination} onClose={close} />
  );
  return { actionsOf, rowActions, dialog };
}

function DestinationDialog({
  command,
  destination,
  onClose,
}: {
  command: Command;
  destination: DestinationView;
  onClose: () => void;
}) {
  if (command === VERIFY_DELIVERY_DESTINATION) {
    return <VerifyDestinationDialog destination={destination} onClose={onClose} />;
  }
  if (command === REPLACE_DELIVERY_DESTINATION) {
    return <ReplaceDestinationDialog destination={destination} onClose={onClose} />;
  }
  return <RetireDestinationDialog destination={destination} onClose={onClose} />;
}

function RetireDestinationDialog({
  destination,
  onClose,
}: {
  destination: DestinationView;
  onClose: () => void;
}) {
  const retire = useCommandMutation<DestinationView, { input: RetireDeliveryDestinationInput }>(
    RETIRE_DELIVERY_DESTINATION,
    { invalidates: DESTINATION_QUERIES },
  );
  return (
    <ReasonConfirmDialog<RetireDeliveryDestinationInput>
      title={`Retire ${destination.label}`}
      confirmLabel="Retire destination"
      variant="danger"
      command={retire}
      input={(reason) => ({
        destinationId: destination.destinationId,
        expectedRevision: destination.revision,
        reason,
      })}
      successTitle={`${destination.label} retired`}
      onClose={onClose}
    >
      <p>
        No further report or notice can be sent to {destination.label}. Retiring cannot be undone;
        open deliveries to it must be redirected to another verified destination.
      </p>
    </ReasonConfirmDialog>
  );
}
