import { useState } from "react";
import type { OrderBillingContext } from "@/generated/OrderBillingContext";
import type { SetOrderBillingContextInput } from "@/generated/SetOrderBillingContextInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { FINANCIAL_QUERIES } from "./contract";
import { EvidenceFields } from "./EvidenceFields";
import { EMPTY_EVIDENCE, type EvidenceValues, evidenceInput } from "./evidenceValues";
import { ChangeSection } from "./FinancialFields";

export function OrderEvidenceDialog({
  context: initialContext,
  revision: initialRevision,
  onClose,
}: {
  context: OrderBillingContext;
  revision: number;
  onClose: () => void;
}) {
  const [context] = useState(initialContext);
  const [revision] = useState(initialRevision);
  const [key] = useState(() => crypto.randomUUID());
  const selection = context.selection;
  return (
    <CommandDialog<EvidenceValues & { source: string; reason: string }, SetOrderBillingContextInput>
      title="Add order financial evidence"
      submitLabel="Save financial evidence"
      command={{ name: "set_order_billing_context", invalidates: FINANCIAL_QUERIES }}
      defaultValues={{ ...EMPTY_EVIDENCE, source: "", reason: "" }}
      toInput={(values) => ({
        orderId: context.orderId,
        expectedRevision: revision,
        expectedBillingVersion: context.version,
        responsibility: selection.responsibility,
        clientId: selection.clientId,
        profileVersion: selection.profileVersion,
        coverageKeys: selection.coverages.map((coverage) => coverage.key),
        useProfileGuarantor: selection.guarantor !== null,
        financialEvidence: [...context.financialEvidence, evidenceInput(values, key)],
        source: values.source.trim(),
        reason: values.reason.trim(),
      })}
      successTitle="Order financial evidence saved"
      onClose={onClose}
      size="wide"
    >
      <EvidenceFields coverages={selection.coverages} />
      <ChangeSection />
    </CommandDialog>
  );
}
