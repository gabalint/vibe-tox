import { useMemo, useState } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { Tone } from "@/components/tones";
import type { DecideFollowOnInput } from "@/generated/DecideFollowOnInput";
import type { Decision } from "@/generated/Decision";
import type { ItemState } from "@/generated/ItemState";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { OrderItemView } from "@/generated/OrderItemView";
import { allowedRowActions, type OfferedAction } from "@/lib/allowedActions";
import { labelOf } from "@/lib/choices";
import { ITEM_KINDS, ITEM_STATES, REFLEX_AUTHORIZATIONS, SEQUENCE_ROLES } from "./labels";
import { type OrderReferences, orderableName, orderedTestName } from "./references";
import { useOrderCommand } from "./useOrderCommand";

const STATE_TONES: Record<ItemState, Tone> = {
  requested: "warning",
  accepted: "neutral",
  declined: "neutral",
  cancelled: "neutral",
};

function itemColumns(references: OrderReferences): DataGridColumn<OrderItemView>[] {
  const test = orderedTestName;
  return [
    { id: "test", header: "Test", cell: test, sortValue: test },
    { id: "kind", header: "Kind", cell: (item) => labelOf(ITEM_KINDS, item.kind), width: 110 },
    {
      id: "role",
      header: "Sequence role",
      // Items migrated from before sequence roles were recorded have none; never assume one.
      cell: (item) =>
        item.sequenceRole === null ? "Not recorded" : labelOf(SEQUENCE_ROLES, item.sequenceRole),
      width: 150,
    },
    {
      id: "state",
      header: "State",
      cell: (item) => (
        <StatusBadge tone={STATE_TONES[item.state]}>{labelOf(ITEM_STATES, item.state)}</StatusBadge>
      ),
      width: 140,
    },
    {
      id: "reflex",
      header: "Reflex testing",
      cell: (item) => {
        const authorization = labelOf(REFLEX_AUTHORIZATIONS, item.reflexAuthorization);
        const scope = item.reflexScopeOrderableIds
          .map((id) => orderableName(references, id))
          .join(", ");
        return scope ? `${authorization}: ${scope}` : authorization;
      },
    },
  ];
}

const getRowId = (item: OrderItemView) => item.itemId;

const FOLLOW_ON_ACTIONS: readonly OfferedAction<Decision>[] = [
  { id: "accept", label: "Accept request" },
  { id: "decline", label: "Decline request", variant: "danger" },
];

interface OrderItemsProps {
  order: OrderDetail;
  references: OrderReferences;
}

/** The order's tests; add-on and reflex requests wait here for a decision (REQ-ORD-002). */
export function OrderItems({ order, references }: OrderItemsProps) {
  const [deciding, setDeciding] = useState<{ item: OrderItemView; decision: Decision } | null>(
    null,
  );
  const columns = useMemo(() => itemColumns(references), [references]);
  const test = orderedTestName;
  // Interim until the engine sends `allowedActions` on OrderItemView: accept and decline are
  // the commands for follow-on requests (add-on and reflex, named by the item's `kind`), so
  // they show only on a requested follow-on; initial tests are decided with the order. The
  // engine checks the item's state and the order again.
  const rowActions = allowedRowActions<OrderItemView, Decision>(FOLLOW_ON_ACTIONS, {
    label: test,
    allowed: (item) =>
      item.kind !== "initial" && item.state === "requested" ? ["accept", "decline"] : [],
    onAction: (decision, item) => setDeciding({ item, decision }),
  });
  return (
    <Section title="Tests">
      <DataGrid
        label="Tests on this order"
        columns={columns}
        rows={order.items}
        getRowId={getRowId}
        rowActions={rowActions}
      />
      {deciding && (
        <DecideFollowOn
          item={deciding.item}
          test={test(deciding.item)}
          decision={deciding.decision}
          onClose={() => setDeciding(null)}
        />
      )}
    </Section>
  );
}

interface DecideFollowOnProps {
  item: OrderItemView;
  test: string;
  decision: Decision;
  onClose: () => void;
}

function DecideFollowOn({ item, test, decision, onClose }: DecideFollowOnProps) {
  const decide = useOrderCommand<DecideFollowOnInput>("decide_follow_on");
  const accepting = decision === "accept";
  return (
    <ReasonConfirmDialog
      title={`${accepting ? "Accept" : "Decline"} the request for ${test}?`}
      confirmLabel={accepting ? "Accept request" : "Decline request"}
      variant={accepting ? "primary" : "danger"}
      command={decide}
      input={(reason) => ({
        itemId: item.itemId,
        expectedRevision: item.revision,
        decision,
        reason,
      })}
      successTitle={`Request ${accepting ? "accepted" : "declined"}`}
      onClose={onClose}
    >
      <p>
        {accepting
          ? "The test is added to the order and can be performed on suitable material."
          : "The test is not performed. The request and this decision are kept."}
      </p>
    </ReasonConfirmDialog>
  );
}
