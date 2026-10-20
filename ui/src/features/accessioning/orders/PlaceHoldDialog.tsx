import { useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { PlaceClarificationHoldInput } from "@/generated/PlaceClarificationHoldInput";
import { checked, choiceOf, optionalText } from "@/lib/choices";
import { DEFICIENCIES } from "./labels";
import { useOrderCommand } from "./useOrderCommand";

interface PlaceHoldDialogProps {
  order: OrderDetail;
  onClose: () => void;
}

/** Holds the order until a deficiency is clarified with the ordering source (REQ-ORD-001). */
export function PlaceHoldDialog({ order, onClose }: PlaceHoldDialogProps) {
  const toast = useToast();
  const place = useOrderCommand<PlaceClarificationHoldInput>("place_clarification_hold");
  const form = useForm<{ deficiency: string | null; note: string }>({
    defaultValues: { deficiency: null, note: "" },
  });
  const onSubmit = form.handleSubmit(({ deficiency, note }) =>
    place.mutate(
      {
        input: {
          orderId: order.orderId,
          expectedRevision: order.revision,
          deficiency: checked(choiceOf(DEFICIENCIES, deficiency)),
          note: optionalText(note),
        },
      },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: "Clarification hold placed" });
          onClose();
        },
      },
    ),
  );
  return (
    <FormDialog
      title="Place clarification hold"
      submitLabel="Place hold"
      form={form}
      onSubmit={onSubmit}
      command={place}
      errorTitle="The hold was not placed"
      onClose={onClose}
    >
      <p>While a hold is open the order cannot be accepted, and its tests cannot be performed.</p>
      <ChoiceInput
        name="deficiency"
        label="What needs clarifying"
        options={DEFICIENCIES}
        isRequired
      />
      <TextInput name="note" label="Note" description="Recorded with the hold." />
    </FormDialog>
  );
}
