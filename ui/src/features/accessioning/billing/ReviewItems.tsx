import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BillingReviewRecord } from "@/generated/BillingReviewRecord";
import type { CreateBillingReviewItemInput } from "@/generated/CreateBillingReviewItemInput";
import type { OrderBillingView } from "@/generated/OrderBillingView";
import type { UpdateBillingReviewItemInput } from "@/generated/UpdateBillingReviewItemInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, labelOf, optionalText } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { FINANCIAL_QUERIES } from "./contract";

import { REVIEW_ELEMENTS as ELEMENTS } from "./labels";

const STATUSES = [
  { id: "open", label: "Open" },
  { id: "resolved", label: "Resolved" },
] as const;

export function ReviewItems({ view }: { view: OrderBillingView }) {
  const [dialog, setDialog] = useState<"create" | BillingReviewRecord | null>(null);
  const close = () => setDialog(null);
  return (
    <Section
      title="Billing follow-up"
      variant="plain"
      actions={
        view.allowedActions.includes("create_billing_review_item") && (
          <Button onPress={() => setDialog("create")}>Open billing follow-up</Button>
        )
      }
    >
      <p className={styles.hint}>
        Financial deficiencies are tracked separately from clinical testing and release decisions.
      </p>
      {!view.reviewItems.length ? (
        <p>No billing follow-up recorded.</p>
      ) : (
        <ul className={styles.list}>
          {view.reviewItems.map((item) => (
            <li key={item.id}>
              <details open={item.status === "open"}>
                <summary>
                  {labelOf(ELEMENTS, item.element)} ·{" "}
                  <StatusBadge tone={item.status === "open" ? "warning" : "neutral"}>
                    {labelOf(STATUSES, item.status)}
                  </StatusBadge>
                </summary>
                <Facts
                  items={[
                    { label: "Original note", value: item.note ?? "None" },
                    { label: "Opened", value: <Timestamp value={item.openedAt} /> },
                    { label: "Owner", value: item.owner ?? "Unassigned" },
                    { label: "Evidence reference", value: item.sourceRef ?? "Not supplied" },
                    { label: "Latest reason", value: item.reason ?? "None" },
                  ]}
                />
                {view.allowedActions.includes("update_billing_review_item") && (
                  <Button onPress={() => setDialog(item)}>Update billing follow-up</Button>
                )}
              </details>
            </li>
          ))}
        </ul>
      )}
      {dialog === "create" && <CreateReviewDialog orderId={view.orderId} onClose={close} />}
      {dialog && dialog !== "create" && <UpdateReviewDialog item={dialog} onClose={close} />}
    </Section>
  );
}

function CreateReviewDialog({ orderId, onClose }: { orderId: string; onClose: () => void }) {
  return (
    <CommandDialog<{ element: string | null; note: string }, CreateBillingReviewItemInput>
      title="Open billing follow-up"
      submitLabel="Open follow-up"
      command={{ name: "create_billing_review_item", invalidates: FINANCIAL_QUERIES }}
      defaultValues={{ element: null, note: "" }}
      toInput={(v) => ({
        orderId,
        element: checked(choiceOf(ELEMENTS, v.element)),
        note: optionalText(v.note),
      })}
      successTitle="Billing follow-up opened"
      onClose={onClose}
    >
      <ChoiceInput
        name="element"
        label="Missing financial evidence"
        options={ELEMENTS}
        isRequired
      />
      <TextInput name="note" label="Source note" />
    </CommandDialog>
  );
}

function UpdateReviewDialog({ item, onClose }: { item: BillingReviewRecord; onClose: () => void }) {
  return (
    <CommandDialog<
      { status: string; owner: string; sourceRef: string; reason: string },
      UpdateBillingReviewItemInput
    >
      title="Update billing follow-up"
      submitLabel="Save follow-up"
      command={{ name: "update_billing_review_item", invalidates: FINANCIAL_QUERIES }}
      defaultValues={{ status: item.status, owner: item.owner ?? "", sourceRef: "", reason: "" }}
      toInput={(v) => ({
        itemId: item.id,
        expectedVersion: item.version,
        status: checked(choiceOf(STATUSES, v.status)),
        owner: optionalText(v.owner),
        sourceRef: optionalText(v.sourceRef),
        reason: v.reason.trim(),
      })}
      successTitle="Billing follow-up updated"
      onClose={onClose}
    >
      <ChoiceInput name="status" label="Follow-up status" options={STATUSES} isRequired />
      <TextInput
        name="owner"
        label="Responsible person or team"
        description="Required to resolve this follow-up."
      />
      <TextInput
        name="sourceRef"
        label="Action evidence reference"
        description="Required to resolve this follow-up."
      />
      <TextInput name="reason" label="Reason for this action" isRequired />
    </CommandDialog>
  );
}
