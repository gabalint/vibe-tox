import { type ReactNode, useState } from "react";
import type { DefaultValues, FieldValues } from "react-hook-form";
import type { PatientBillingContent } from "@/generated/PatientBillingContent";
import type { PatientBillingProfileView } from "@/generated/PatientBillingProfileView";
import type { SetPatientBillingProfileInput } from "@/generated/SetPatientBillingProfileInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { contactInput, contactValues } from "./contactValues";
import { FINANCIAL_QUERIES } from "./contract";
import { ChangeSection } from "./FinancialFields";

export interface ProfileDialogProps {
  view: PatientBillingProfileView;
  onClose: () => void;
}
export const emptyBillingContent = (): PatientBillingContent => ({
  contact: contactInput(contactValues()),
  guarantor: null,
  coverages: [],
  financialEvidence: [],
});

/** Each focused editor replaces one part while retaining every other supplied financial fact. */
export function PatientProfileDialog<
  Values extends FieldValues & { source: string; reason: string },
>({
  view,
  onClose,
  title,
  defaultValues,
  update,
  children,
}: ProfileDialogProps & {
  title: string;
  defaultValues: DefaultValues<Values>;
  update: (content: PatientBillingContent, values: Values) => PatientBillingContent;
  children: ReactNode;
}) {
  const [base] = useState(view);
  return (
    <CommandDialog<Values, SetPatientBillingProfileInput>
      title={title}
      submitLabel="Save billing details"
      command={{ name: "set_patient_billing_profile", invalidates: FINANCIAL_QUERIES }}
      defaultValues={defaultValues}
      toInput={(values) => ({
        subjectId: base.subjectId,
        expectedVersion: base.profile?.version ?? null,
        content: update(base.profile?.content ?? emptyBillingContent(), values),
        source: values.source.trim(),
        reason: values.reason.trim(),
      })}
      successTitle="Billing details saved"
      onClose={onClose}
      size="wide"
    >
      {children}
      <ChangeSection />
    </CommandDialog>
  );
}
