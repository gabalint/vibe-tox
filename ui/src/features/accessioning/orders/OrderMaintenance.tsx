import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { CheckList, ChoiceInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import type { AmendOrderEvidenceInput } from "@/generated/AmendOrderEvidenceInput";
import type { DecideCancellationInput } from "@/generated/DecideCancellationInput";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { RequestCancellationInput } from "@/generated/RequestCancellationInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { useHasCapability } from "@/lib/commands";
import { ORDER_QUERIES } from "./contract";
import { EvidenceFields } from "./EvidenceFields";
import { EMPTY_ORDER, type OrderFormValues, toEvidence } from "./orderValues";
import { orderedTestName } from "./references";

export function OrderMaintenance({ order }: { order: OrderDetail }) {
  const canEdit = useHasCapability("enter_orders");
  const [action, setActionState] = useState<"amend" | "cancel" | string | null>(null);
  const [reviewed, setReviewed] = useState(order);
  const setAction = (next: typeof action) => {
    setReviewed(order);
    setActionState(next);
  };
  const close = () => setActionState(null);
  const pendingItems = new Set(
    order.cancellations.filter((row) => row.state === "requested").map((row) => row.itemId),
  );
  const cancellable = order.items.filter(
    (row) => row.state === "accepted" && !pendingItems.has(row.itemId),
  );
  const deciding = reviewed.cancellations.find((row) => row.requestId === action);
  return (
    <Section
      title="Order changes"
      actions={
        canEdit && (
          <>
            {order.allowedActions.includes("amend_evidence") && (
              <Button onPress={() => setAction("amend")}>Amend supplied evidence</Button>
            )}
            {cancellable.length > 0 && (
              <Button onPress={() => setAction("cancel")}>Request cancellation</Button>
            )}
          </>
        )
      }
    >
      <p>
        Amendments retain earlier evidence. Cancellation is a request followed by an explicit
        decision; existing observations and reports are preserved.
      </p>
      <DataGrid
        label="Cancellation history"
        rows={order.cancellations}
        getRowId={(row) => row.requestId}
        columns={[
          {
            id: "test",
            header: "Test",
            cell: (row) => {
              const item = order.items.find((each) => each.itemId === row.itemId);
              return item ? orderedTestName(item) : "Unavailable test";
            },
          },
          { id: "by", header: "Requested by", cell: (row) => row.requestedBy },
          { id: "reason", header: "Request reason", cell: (row) => row.reason },
          { id: "state", header: "Decision", cell: (row) => row.state },
          {
            id: "decisionReason",
            header: "Decision reason",
            cell: (row) => row.decisionReason ?? "Pending",
          },
          {
            id: "action",
            header: "Action",
            cell: (row) =>
              row.state === "requested" && canEdit ? (
                <Button onPress={() => setAction(row.requestId)}>Decide cancellation</Button>
              ) : null,
          },
        ]}
      />
      {action === "amend" && <AmendEvidence order={reviewed} onClose={close} />}
      {action === "cancel" && (
        <CommandDialog<
          { itemId: string; requestedBy: string; reason: string },
          RequestCancellationInput
        >
          title="Request test cancellation"
          submitLabel="Record cancellation request"
          command={{ name: "request_cancellation", invalidates: ORDER_QUERIES }}
          defaultValues={{ itemId: "", requestedBy: "", reason: "" }}
          toInput={(v) => ({
            itemId: v.itemId,
            requestedBy: v.requestedBy.trim(),
            reason: v.reason.trim(),
          })}
          successTitle="Cancellation requested"
          onClose={close}
        >
          <ChoiceInput
            name="itemId"
            label="Test to cancel"
            options={cancellable.map((row) => ({ id: row.itemId, label: orderedTestName(row) }))}
            isRequired
          />
          <TextInput name="requestedBy" label="Requester as supplied" isRequired />
          <TextInput name="reason" label="Reason" isRequired />
        </CommandDialog>
      )}
      {deciding && (
        <CommandDialog<{ decision: "accept" | "decline"; reason: string }, DecideCancellationInput>
          title="Decide cancellation"
          submitLabel="Record decision"
          command={{
            name: "decide_cancellation",
            invalidates: [
              ...ORDER_QUERIES,
              "get_specimen_results",
              "get_report_workspace",
              "list_review_queue",
            ],
          }}
          defaultValues={{ reason: "" }}
          toInput={(v) => ({
            requestId: deciding.requestId,
            expectedRevision: deciding.revision,
            decision: v.decision,
            reason: v.reason.trim(),
          })}
          successTitle="Cancellation decision recorded"
          onClose={close}
        >
          <p>
            Request: {deciding.reason}. Accepting stops further work for this item. Review
            issued-report impacts separately; issued copies remain unchanged.
          </p>
          <ChoiceInput
            name="decision"
            label="Decision"
            options={[
              { id: "accept", label: "Accept cancellation" },
              { id: "decline", label: "Decline cancellation" },
            ]}
            isRequired
          />
          <TextInput name="reason" label="Decision reason" isRequired />
        </CommandDialog>
      )}
    </Section>
  );
}

function AmendEvidence({ order, onClose }: { order: OrderDetail; onClose: () => void }) {
  const options = [
    ...order.evidence.authority.map((r) => ({
      ...r,
      label: `Authority: ${r.kind} · ${r.documentRef ?? "no reference"}`,
    })),
    ...order.evidence.diagnoses.map((r) => ({
      ...r,
      label: `Diagnosis: ${r.code} · ${r.documentRef}`,
    })),
    ...order.evidence.medications.map((r) => ({ ...r, label: `Medication: ${r.receivedText}` })),
    ...order.evidence.pointOfCareResults.map((r) => ({
      ...r,
      label: `Point-of-care: ${r.deviceOrTest} · ${r.resultText}`,
    })),
  ]
    .filter((r) => r.removedInVersion === null)
    .map((r) => ({ id: r.evidenceId, label: r.label }));
  return (
    <CommandDialog<
      OrderFormValues & { removeEvidenceIds: string[]; reason: string },
      AmendOrderEvidenceInput
    >
      title="Amend supplied evidence"
      submitLabel="Save evidence amendment"
      command={{
        name: "amend_order_evidence",
        invalidates: [...ORDER_QUERIES, "get_report_workspace", "get_specimen_results"],
      }}
      defaultValues={{ ...EMPTY_ORDER, removeEvidenceIds: [], reason: "" }}
      toInput={(v) => ({
        orderId: order.orderId,
        expectedRevision: order.revision,
        add: toEvidence(v),
        removeEvidenceIds: v.removeEvidenceIds,
        reason: v.reason.trim(),
      })}
      successTitle="Evidence amendment saved"
      onClose={onClose}
    >
      <p>
        Enter only newly supplied evidence. Select superseded entries explicitly; they remain in
        history. Do not infer diagnoses, medical necessity or medications.
      </p>
      <EvidenceFields />
      <CheckList
        name="removeEvidenceIds"
        legend="Supersede these existing entries"
        options={options}
      />
      <TextInput name="reason" label="Reason and source for amendment" isRequired />
    </CommandDialog>
  );
}
