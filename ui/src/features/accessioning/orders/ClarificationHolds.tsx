import { Button } from "@/components/Button";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { StatusBadge } from "@/components/StatusBadge";
import type { ClarificationHoldView } from "@/generated/ClarificationHoldView";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { ResolveClarificationHoldInput } from "@/generated/ResolveClarificationHoldInput";
import { labelOf } from "@/lib/choices";
import styles from "../accessioning.module.css";
import { HoldsSection } from "../HoldsSection";
import { DEFICIENCIES } from "./labels";
import { useOrderCommand } from "./useOrderCommand";

/**
 * Clarification holds on the order (REQ-ORD-001). While one is open the order cannot be
 * accepted and none of its tests can be performed.
 */
export function ClarificationHolds({ order }: { order: OrderDetail }) {
  return (
    <HoldsSection
      title="Clarification holds"
      emptyText="No clarification holds."
      holds={order.holds}
      holdId={(hold) => hold.holdId}
      renderHold={(hold, resolve) => (
        <div className={styles.listRow}>
          <StatusBadge tone={hold.state === "open" ? "hold" : "neutral"}>
            {hold.state === "open" ? "Open" : "Resolved"}
          </StatusBadge>
          <span>{labelOf(DEFICIENCIES, hold.deficiency)}</span>
          {hold.origin === "standing_order_check" && (
            <span className={styles.hint}>Placed by the standing-order check</span>
          )}
          {hold.note && <span className={styles.hint}>{hold.note}</span>}
          {hold.state === "open" && <Button onPress={resolve}>Resolve</Button>}
        </div>
      )}
      renderResolution={(hold, close) => <ResolveHold hold={hold} onClose={close} />}
    />
  );
}

function ResolveHold({ hold, onClose }: { hold: ClarificationHoldView; onClose: () => void }) {
  const resolve = useOrderCommand<ResolveClarificationHoldInput>("resolve_clarification_hold");
  return (
    <ReasonConfirmDialog
      title="Resolve this hold?"
      confirmLabel="Resolve hold"
      variant="primary"
      command={resolve}
      input={(reason) => ({ holdId: hold.holdId, expectedRevision: hold.revision, reason })}
      successTitle="Hold resolved"
      onClose={onClose}
    >
      <p>
        {labelOf(DEFICIENCIES, hold.deficiency)}: record how it was clarified, such as the signed
        requisition received from the clinic.
      </p>
    </ReasonConfirmDialog>
  );
}
