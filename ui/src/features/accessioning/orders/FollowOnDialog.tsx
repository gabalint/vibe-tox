import { useForm, useWatch } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput, RecordPickerInput, TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import type { ItemKind } from "@/generated/ItemKind";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { RequestFollowOnInput } from "@/generated/RequestFollowOnInput";
import { type Choice, checked, choiceOf, optionalText } from "@/lib/choices";
import { SEQUENCE_ROLES } from "./labels";
import { pointOfCareText } from "./OrderEvidence";
import { type OrderReferences, orderableLabel, orderedTestName } from "./references";
import { useOrderCommand } from "./useOrderCommand";

const FOLLOW_ON_KINDS: readonly Choice<Exclude<ItemKind, "initial">>[] = [
  { id: "add_on", label: "Add-on, with new authority" },
  { id: "reflex", label: "Reflex, under an authorized test's reflex scope" },
];

interface FollowOnValues {
  kind: string | null;
  orderableVersionId: string | null;
  sequenceRole: string | null;
  parentItemId: string | null;
  pocResultId: string | null;
  authorityReference: string;
  reason: string;
}

interface FollowOnDialogProps {
  order: OrderDetail;
  references: OrderReferences;
  onClose: () => void;
}

/**
 * Requests an add-on or reflex test (REQ-ORD-002, REQ-ORD-003). It is a request: someone
 * decides it separately, and the engine refuses a reflex outside the authorized scope. A
 * reflex here names the client's point-of-care result that triggered it; a reflex from a
 * laboratory result comes from the results module, which verifies that result.
 */
export function FollowOnDialog({ order, references, onClose }: FollowOnDialogProps) {
  const toast = useToast();
  const request = useOrderCommand<RequestFollowOnInput>("request_follow_on");
  const form = useForm<FollowOnValues>({
    defaultValues: {
      kind: null,
      orderableVersionId: null,
      sequenceRole: null,
      parentItemId: null,
      pocResultId: null,
      authorityReference: "",
      reason: "",
    },
  });
  const kind = useWatch({ control: form.control, name: "kind" });
  const onSubmit = form.handleSubmit((values) =>
    request.mutate(
      { input: toInput(order, values) },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: "Request recorded; it awaits a decision" });
          onClose();
        },
      },
    ),
  );
  const tests = references.orderables.map((each) => ({ id: each.id, label: orderableLabel(each) }));
  return (
    <FormDialog
      title="Request add-on or reflex"
      submitLabel="Record request"
      form={form}
      onSubmit={onSubmit}
      command={request}
      errorTitle="The request was not recorded"
      onClose={onClose}
    >
      <ChoiceInput name="kind" label="Kind of request" options={FOLLOW_ON_KINDS} isRequired />
      <RecordPickerInput name="orderableVersionId" label="Test" options={tests} isRequired />
      <ChoiceInput
        name="sequenceRole"
        label="Testing sequence role"
        options={SEQUENCE_ROLES}
        description="As the request states it."
        isRequired
      />
      {kind === "reflex" && <ReflexTriggerFields order={order} />}
      <TextInput
        name="authorityReference"
        label="Authority reference"
        description="The document or call that authorizes the test."
      />
      <ReasonField />
    </FormDialog>
  );
}

/** The parent test and the client's point-of-care result that triggered a reflex (REQ-ORD-003). */
function ReflexTriggerFields({ order }: Pick<FollowOnDialogProps, "order">) {
  const parentItems = order.items.map((item) => ({
    id: item.itemId,
    label: orderedTestName(item),
  }));
  // Only point-of-care results still on the order can trigger a reflex.
  const pocResults = order.evidence.pointOfCareResults
    .filter((result) => result.removedInVersion === null)
    .map((result) => ({ id: result.evidenceId, label: pointOfCareText(result) }));
  return (
    <>
      <ChoiceInput name="parentItemId" label="Reflex from test" options={parentItems} isRequired />
      <ChoiceInput
        name="pocResultId"
        label="Triggering point-of-care result"
        options={pocResults}
        description={
          pocResults.length === 0
            ? "This order has no point-of-care result to trigger a reflex."
            : "As the client supplied it on this order."
        }
        isRequired
      />
    </>
  );
}

function toInput(order: OrderDetail, values: FollowOnValues): RequestFollowOnInput {
  const kind = checked(choiceOf(FOLLOW_ON_KINDS, values.kind));
  const reflex = kind === "reflex";
  return {
    orderId: order.orderId,
    expectedRevision: order.revision,
    kind,
    orderableVersionId: checked(values.orderableVersionId),
    sequenceRole: checked(choiceOf(SEQUENCE_ROLES, values.sequenceRole)),
    parentItemId: reflex ? values.parentItemId : null,
    authorityReference: optionalText(values.authorityReference),
    triggerSource: reflex ? "client_point_of_care" : null,
    triggerRef: null,
    pocResultId: reflex ? values.pocResultId : null,
    reason: values.reason.trim(),
  };
}
