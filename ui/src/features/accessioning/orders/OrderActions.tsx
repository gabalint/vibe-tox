import { useState } from "react";
import { useNavigate } from "react-router";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { useToast } from "@/components/Toast";
import type { AcceptOrderInput } from "@/generated/AcceptOrderInput";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { RejectOrderInput } from "@/generated/RejectOrderInput";
import { allowedActionItems, type OfferedAction } from "@/lib/allowedActions";
import type { useOrderAccessionContext } from "../useOrderAccessionContext";
import { ORDER_ACTIONS } from "./contract";
import { FollowOnDialog } from "./FollowOnDialog";
import { PlaceHoldDialog } from "./PlaceHoldDialog";
import type { OrderReferences } from "./references";
import { useOrderCommand } from "./useOrderCommand";

type OrderAction = (typeof ORDER_ACTIONS)[keyof typeof ORDER_ACTIONS];

/** The order-level decisions, in the order offered, by the engine's names. */
const DECISIONS: readonly OfferedAction<OrderAction>[] = [
  { id: ORDER_ACTIONS.accept, label: "Accept order" },
  { id: ORDER_ACTIONS.placeHold, label: "Place clarification hold" },
  { id: ORDER_ACTIONS.requestFollowOn, label: "Request add-on or reflex" },
  { id: ORDER_ACTIONS.reject, label: "Reject order", variant: "danger" },
];

interface OrderActionsProps {
  order: OrderDetail;
  references: OrderReferences | null;
  accessionContext: ReturnType<typeof useOrderAccessionContext>;
  receiveNext: boolean;
}

/**
 * The order-level decisions the engine allows now, from its `allowedActions`: the first as
 * the primary button, two more as buttons, the rest under "More". Receiving is offered only
 * by the engine's accession context; the command rechecks eligibility at submission.
 */
export function OrderActions({
  order,
  references,
  accessionContext,
  receiveNext,
}: OrderActionsProps) {
  const navigate = useNavigate();
  const [dialog, setDialog] = useState<OrderAction | null>(null);
  const close = () => setDialog(null);
  const decisions = allowedActionItems(
    DECISIONS.filter((decision) => decision.id !== ORDER_ACTIONS.requestFollowOn || references),
    order.allowedActions,
    setDialog,
  );
  const createAccession: ActionItem = {
    id: "create_accession",
    label: "Receive specimens",
    onAction: () => navigate(`/orders/${order.orderId}/accession`),
  };
  const items =
    !accessionContext.isFetching &&
    !accessionContext.isError &&
    accessionContext.data?.allowedActions.includes("create_accession")
      ? [createAccession, ...decisions]
      : decisions;
  const continueReceiving = async () => {
    if (!receiveNext) return;
    const refreshed = await accessionContext.refetch();
    if (!refreshed.isError && refreshed.data?.allowedActions.includes("create_accession")) {
      navigate(`/orders/${order.orderId}/accession`);
    }
  };
  return (
    <>
      <RecordActions items={items} />
      {dialog === ORDER_ACTIONS.accept && (
        <AcceptOrder order={order} onClose={close} onAccepted={continueReceiving} />
      )}
      {dialog === ORDER_ACTIONS.reject && <RejectOrder order={order} onClose={close} />}
      {dialog === ORDER_ACTIONS.placeHold && <PlaceHoldDialog order={order} onClose={close} />}
      {dialog === ORDER_ACTIONS.requestFollowOn && references && (
        <FollowOnDialog order={order} references={references} onClose={close} />
      )}
    </>
  );
}

interface OrderDialogProps {
  order: OrderDetail;
  onClose: () => void;
}

function AcceptOrder({
  order,
  onClose,
  onAccepted,
}: OrderDialogProps & { onAccepted: () => Promise<void> }) {
  const toast = useToast();
  const accept = useOrderCommand<AcceptOrderInput>("accept_order");
  const confirm = () =>
    accept.mutate(
      { input: { orderId: order.orderId, expectedRevision: order.revision } },
      {
        onSuccess: async (updated) => {
          // A failed standing-order check places a hold and leaves the order received.
          toast.show(
            updated.status === "accepted"
              ? { tone: "success", title: "Order accepted" }
              : {
                  tone: "warning",
                  title: "The order was not accepted",
                  description: "A clarification hold was placed. See the holds on the order.",
                },
          );
          if (updated.status === "accepted") await onAccepted();
          onClose();
        },
      },
    );
  return (
    <ConfirmDialog
      title="Accept this order?"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel="Accept order"
      onConfirm={confirm}
      isPending={accept.isPending}
      variant="primary"
    >
      <p>
        The ordered tests become ready for accessioning. An accepted order cannot be rejected later;
        tests can still be cancelled through a cancellation request.
      </p>
      {accept.error && <ErrorState title="The order was not accepted" error={accept.error} />}
    </ConfirmDialog>
  );
}

function RejectOrder({ order, onClose }: OrderDialogProps) {
  const reject = useOrderCommand<RejectOrderInput>("reject_order");
  return (
    <ReasonConfirmDialog
      title="Reject this order?"
      confirmLabel="Reject order"
      variant="danger"
      command={reject}
      input={(reason) => ({ orderId: order.orderId, expectedRevision: order.revision, reason })}
      successTitle="Order rejected"
      onClose={onClose}
    >
      <p>
        None of the order's tests will be performed. The order and its evidence are kept, and the
        rejection cannot be undone.
      </p>
    </ReasonConfirmDialog>
  );
}
