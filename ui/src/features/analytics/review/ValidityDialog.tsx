import { useFormContext, useWatch } from "react-hook-form";
import { CheckList, ChoiceInput, TextInput } from "@/components/FormFields";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { RecordValidityAssessmentInput } from "@/generated/RecordValidityAssessmentInput";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import type { ValidityAssessmentView } from "@/generated/ValidityAssessmentView";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, optionalText } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { GET_VALIDITY_POLICY, LIST_VALIDITY_POLICIES, RESULT_QUERIES } from "../contract";
import { resultValueText } from "../resultLabels";

interface ValidityValues {
  policyVersionId: string | null;
  categoryCode: string | null;
  evidenceRevisionIds: string[];
  reason: string;
}

/**
 * Records the specimen's validity category under an approved policy version, citing the
 * validity results it rests on. The category is chosen by the reviewer, never derived here.
 */
export function ValidityDialog({
  results,
  onClose,
}: {
  results: SpecimenResults;
  onClose: () => void;
}) {
  const current = results.currentValidity;
  const evidence = results.lines.flatMap((line) =>
    line.purpose === "validity" && line.current
      ? [
          {
            id: line.current.revisionId,
            label: `${line.analyteName}: ${resultValueText(line.current)}`,
          },
        ]
      : [],
  );
  return (
    <CommandDialog<ValidityValues, RecordValidityAssessmentInput, ValidityAssessmentView>
      title="Record validity assessment"
      submitLabel="Record assessment"
      command={{ name: "record_validity_assessment", invalidates: RESULT_QUERIES }}
      defaultValues={{
        policyVersionId: null,
        categoryCode: null,
        evidenceRevisionIds: [],
        reason: "",
      }}
      toInput={(values) => ({
        specimenId: results.specimenId,
        policyVersionId: checked(values.policyVersionId),
        categoryCode: checked(values.categoryCode),
        evidenceRevisionIds: values.evidenceRevisionIds,
        supersedesId: current?.assessmentId ?? null,
        reason: optionalText(values.reason),
      })}
      successTitle="Validity assessment recorded"
      onClose={onClose}
    >
      <PolicyChoice specimenTypeCode={results.specimenTypeCode} />
      <CategoryChoice />
      <CheckList
        name="evidenceRevisionIds"
        legend="Validity results relied on"
        options={evidence}
        requiredMessage="Choose the validity results this assessment rests on."
      />
      <TextInput
        name="reason"
        label="Reason"
        description={current ? "Replaces the current assessment; audited." : "Optional; audited."}
        isRequired={current !== null}
      />
    </CommandDialog>
  );
}

function PolicyChoice({ specimenTypeCode }: { specimenTypeCode: string }) {
  const policies = useCommandQuery<CatalogVersionSummary[]>(LIST_VALIDITY_POLICIES, {
    input: { specimenTypeCode },
  });
  const options = (policies.data ?? [])
    .filter((policy) => policy.status === "approved")
    .map((policy) => ({ id: policy.id, label: `Policy version ${policy.version}` }));
  return (
    <ChoiceInput
      name="policyVersionId"
      label="Validity policy"
      options={options}
      description="Approved versions for this specimen type."
      isRequired
    />
  );
}

function CategoryChoice() {
  const { control } = useFormContext<ValidityValues>();
  const policyVersionId = useWatch({ control, name: "policyVersionId" });
  if (policyVersionId === null) return null;
  return <PolicyCategories policyVersionId={policyVersionId} />;
}

function PolicyCategories({ policyVersionId }: { policyVersionId: string }) {
  const policy = useCommandQuery<ValidityPolicyView>(GET_VALIDITY_POLICY, {
    input: { policyVersionId },
  });
  const options = (policy.data?.categories ?? []).map((category) => ({
    id: category.code,
    label: category.label,
  }));
  return <ChoiceInput name="categoryCode" label="Category" options={options} isRequired />;
}
