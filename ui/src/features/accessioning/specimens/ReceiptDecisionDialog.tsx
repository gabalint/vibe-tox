import { useState } from "react";
import { useForm, useFormContext, useWatch } from "react-hook-form";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { FormDialog } from "@/components/FormDialog";
import { CheckList, ChoiceInput, TextInput } from "@/components/FormFields";
import { useToast } from "@/components/Toast";
import type { AccessionSpecimenView } from "@/generated/AccessionSpecimenView";
import type { DiscrepancyInput } from "@/generated/DiscrepancyInput";
import type { ReceiptDiscrepancyCode } from "@/generated/ReceiptDiscrepancyCode";
import type { RecordReceiptDecisionInput } from "@/generated/RecordReceiptDecisionInput";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { LabelCheckDialog } from "./LabelCheckDialog";
import { DISCREPANCIES, RECEIPT_DECISIONS } from "./labels";
import { useAccessionCommand } from "./useAccessionCommand";

interface ReceiptValues {
  decision: string | null;
  discrepancyCodes: string[];
  discrepancyNotes: Partial<Record<string, string>>;
  policyReference: string;
  scope: string;
  reason: string;
}

interface ReceiptDecisionDialogProps {
  specimen: AccessionSpecimenView;
  title: string;
  onClose: () => void;
}

/**
 * Records whether a specimen is accepted for testing, with any discrepancies (REQ-SPC-002).
 * The person first scans or types the label of one of the specimen's containers, so the
 * decision is made with the right material in hand.
 */
export function ReceiptDecisionDialog({ specimen, title, onClose }: ReceiptDecisionDialogProps) {
  const [checkedLabel, setCheckedLabel] = useState<string | null>(null);
  if (checkedLabel === null) {
    return (
      <LabelCheckDialog
        title={`Receipt decision for ${title}`}
        expected={`a container of ${title}`}
        labelCodes={specimen.containers.map((container) => container.labelCode)}
        onConfirmed={setCheckedLabel}
        onClose={onClose}
      />
    );
  }
  return (
    <ReceiptDecisionForm
      specimen={specimen}
      title={title}
      checkedLabel={checkedLabel}
      onClose={onClose}
    />
  );
}

const EMPTY_RECEIPT: ReceiptValues = {
  decision: null,
  discrepancyCodes: [],
  discrepancyNotes: {},
  policyReference: "",
  scope: "",
  reason: "",
};

/**
 * The decision itself. A conditional acceptance names the lab policy and its scope; a hold or
 * rejection gives a reason. Rejection is final, so it is confirmed separately.
 */
function ReceiptDecisionForm({
  specimen,
  title,
  checkedLabel,
  onClose,
}: ReceiptDecisionDialogProps & { checkedLabel: string }) {
  const toast = useToast();
  const record = useAccessionCommand<RecordReceiptDecisionInput>("record_receipt_decision");
  const [pendingRejection, setPendingRejection] = useState<RecordReceiptDecisionInput | null>(null);
  const form = useForm<ReceiptValues>({ defaultValues: EMPTY_RECEIPT });
  const send = (input: RecordReceiptDecisionInput) =>
    record.mutate(
      { input },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: `Receipt decision recorded for ${title}` });
          onClose();
        },
      },
    );
  const onSubmit = form.handleSubmit((values) => {
    const input = toInput(specimen, values);
    if (input.decision === "rejected") setPendingRejection(input);
    else send(input);
  });

  if (pendingRejection) {
    return (
      <ConfirmDialog
        title={`Reject ${title}?`}
        isOpen
        onOpenChange={(isOpen) => {
          if (!isOpen) setPendingRejection(null);
        }}
        confirmLabel="Reject specimen"
        onConfirm={() => send(pendingRejection)}
        isPending={record.isPending}
      >
        <p>
          The specimen will not be tested, and the rejection cannot be changed afterwards. Its
          containers and this decision are kept.
        </p>
        {record.error && <ErrorState title="The decision was not recorded" error={record.error} />}
      </ConfirmDialog>
    );
  }
  return (
    <FormDialog
      title={`Receipt decision for ${title}`}
      submitLabel="Record decision"
      form={form}
      onSubmit={onSubmit}
      command={record}
      errorTitle="The decision was not recorded"
      onClose={onClose}
    >
      <p>Container {checkedLabel} checked.</p>
      <ChoiceInput name="decision" label="Decision" options={RECEIPT_DECISIONS} isRequired />
      <CheckList name="discrepancyCodes" legend="Discrepancies found" options={DISCREPANCIES} />
      <DecisionDetails />
    </FormDialog>
  );
}

/** The fields the chosen decision and discrepancies call for. */
function DecisionDetails() {
  const { control } = useFormContext<ReceiptValues>();
  const [decision, codes] = useWatch({ control, name: ["decision", "discrepancyCodes"] });
  return (
    <>
      {DISCREPANCIES.filter((each) => codes.includes(each.id)).map((each) => (
        <TextInput
          key={each.id}
          name={`discrepancyNotes.${each.id}`}
          label={`Note on ${each.label.toLowerCase()}`}
          description="Retained receipt evidence. Report preparation separately reviews whether this description belongs in the patient report."
        />
      ))}
      {decision === "accepted_conditionally" && (
        <>
          <TextInput name="policyReference" label="Laboratory policy reference" isRequired />
          <TextInput
            name="scope"
            label="Scope of the condition"
            description="Which tests or results the condition covers."
            isRequired
          />
        </>
      )}
      {(decision === "held" || decision === "rejected") && (
        <TextInput
          name="reason"
          label="Reason"
          description="Recorded in the audit trail."
          isRequired
        />
      )}
    </>
  );
}

function toInput(
  specimen: AccessionSpecimenView,
  values: ReceiptValues,
): RecordReceiptDecisionInput {
  const decision = checked(choiceOf(RECEIPT_DECISIONS, values.decision));
  const discrepancies = DISCREPANCIES.filter((each) =>
    values.discrepancyCodes.includes(each.id),
  ).map((each) => discrepancy(each.id, values.discrepancyNotes[each.id]));
  const conditional = decision === "accepted_conditionally";
  const needsReason = decision === "held" || decision === "rejected";
  return {
    specimenId: specimen.specimenId,
    expectedRevision: specimen.revision,
    decision,
    discrepancies,
    policyReference: conditional ? values.policyReference.trim() : undefined,
    scope: conditional ? values.scope.trim() : undefined,
    reason: needsReason ? values.reason.trim() : undefined,
  };
}

function discrepancy(code: ReceiptDiscrepancyCode, note: string | undefined): DiscrepancyInput {
  return { code, note: omittedIfBlank(note ?? "") };
}
