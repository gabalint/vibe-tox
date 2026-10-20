import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { LotView } from "@/generated/LotView";
import type { WithdrawLotInput } from "@/generated/WithdrawLotInput";
import { useCommandMutation } from "@/lib/commands";
import { LOT_QUERIES } from "../contract";

/** Withdraws a lot with the reason; runs that already use it keep the record (REQ-ANA-007). */
export function WithdrawLotDialog({ lot, onClose }: { lot: LotView; onClose: () => void }) {
  const withdraw = useCommandMutation<LotView, { input: WithdrawLotInput }>("withdraw_lot", {
    invalidates: LOT_QUERIES,
  });
  return (
    <ReasonConfirmDialog<WithdrawLotInput>
      title={`Withdraw lot ${lot.lotNumber}`}
      confirmLabel="Withdraw lot"
      variant="danger"
      command={withdraw}
      input={(reason) => ({ lotId: lot.lotId, expectedRevision: lot.revision, reason })}
      successTitle={`Lot ${lot.lotNumber} withdrawn`}
      onClose={onClose}
    >
      <p>
        {lot.productName} lot {lot.lotNumber} can no longer be used on new runs. Runs that already
        use it keep the record.
      </p>
    </ReasonConfirmDialog>
  );
}
