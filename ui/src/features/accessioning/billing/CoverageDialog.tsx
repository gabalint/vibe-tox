import { useWatch } from "react-hook-form";
import { FieldGrid, FieldSection } from "@/components/Form";
import { ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import type { PatientCoverage } from "@/generated/PatientCoverage";
import { optionalText } from "@/lib/choices";
import { partyInput, partyValues } from "./contactValues";
import { PartyFields } from "./FinancialFields";
import { PatientProfileDialog, type ProfileDialogProps } from "./PatientProfileDialog";

const SUBSCRIBER = [
  { id: "recorded", label: "Subscriber supplied" },
  { id: "not_supplied", label: "Not supplied" },
];

export function CoverageDialog({
  coverage,
  ...props
}: ProfileDialogProps & { coverage?: PatientCoverage }) {
  const key = coverage?.key ?? crypto.randomUUID();
  return (
    <PatientProfileDialog
      {...props}
      title={coverage ? "Correct coverage" : "Add coverage"}
      defaultValues={{
        payerName: coverage?.payerName ?? "",
        payerId: coverage?.payerId ?? "",
        planName: coverage?.planName ?? "",
        memberId: coverage?.memberId ?? "",
        groupNumber: coverage?.groupNumber ?? "",
        effectiveFrom: coverage?.effectiveFrom ?? "",
        effectiveUntil: coverage?.effectiveUntil ?? "",
        coverageSource: coverage?.source ?? "",
        verifiedOn: coverage?.verifiedOn ?? "",
        subscriberStatus: coverage?.subscriber ? "recorded" : null,
        subscriber: partyValues(coverage?.subscriber),
        source: "",
        reason: "",
      }}
      update={(content, v) => {
        const next: PatientCoverage = {
          key,
          payerName: v.payerName.trim(),
          payerId: optionalText(v.payerId),
          planName: optionalText(v.planName),
          memberId: optionalText(v.memberId),
          groupNumber: optionalText(v.groupNumber),
          effectiveFrom: optionalText(v.effectiveFrom),
          effectiveUntil: optionalText(v.effectiveUntil),
          source: v.coverageSource.trim(),
          verifiedOn: optionalText(v.verifiedOn),
          subscriber: v.subscriberStatus === "recorded" ? partyInput(v.subscriber) : null,
        };
        return {
          ...content,
          coverages: coverage
            ? content.coverages.map((each) => (each.key === key ? next : each))
            : [...content.coverages, next],
        };
      }}
    >
      <FieldSection title="Coverage">
        <FieldGrid>
          <TextInput name="payerName" label="Payer name" isRequired />
          <TextInput name="payerId" label="Payer identifier" />
          <TextInput name="planName" label="Plan name" />
          <TextInput name="memberId" label="Member identifier" />
          <TextInput name="groupNumber" label="Group number" />
          <TextInput name="effectiveFrom" label="Coverage starts" format={DATE_FORMAT} />
          <TextInput name="effectiveUntil" label="Coverage ends" format={DATE_FORMAT} />
          <TextInput name="verifiedOn" label="Coverage checked on" format={DATE_FORMAT} />
          <TextInput name="coverageSource" label="Coverage information source" isRequired />
        </FieldGrid>
      </FieldSection>
      <FieldSection title="Subscriber">
        <FieldGrid>
          <ChoiceInput
            name="subscriberStatus"
            label="Subscriber details"
            options={SUBSCRIBER}
            isRequired
          />
        </FieldGrid>
        <SubscriberFields />
      </FieldSection>
    </PatientProfileDialog>
  );
}

function SubscriberFields() {
  return useWatch({ name: "subscriberStatus" }) === "recorded" ? (
    <PartyFields prefix="subscriber" label="Subscriber" />
  ) : null;
}
