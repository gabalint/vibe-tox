import { useFieldArray, useForm, useFormContext, useWatch } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { DraftValidityPolicyVersion } from "@/generated/DraftValidityPolicyVersion";
import type { UpdateValidityPolicyDraft } from "@/generated/UpdateValidityPolicyDraft";
import type { ValidityPolicyContent } from "@/generated/ValidityPolicyContent";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { checked, choiceOf } from "@/lib/choices";
import { VALIDITY_POLICY_QUERIES } from "../contract";
import { DraftDialog, type DraftMode, draftBase } from "../DraftDialog";
import { VALIDITY_ACTIONS } from "../labels";

interface CategoryValues {
  code: string;
  label: string;
  action: string | null;
  qualifierText: string;
}

interface ValidityFormValues {
  categories: CategoryValues[];
}

const EMPTY_CATEGORY: CategoryValues = { code: "", label: "", action: null, qualifierText: "" };

interface ValidityDraftDialogProps {
  specimenTypeId: string;
  mode: DraftMode<ValidityPolicyView>;
  onSaved: (draft: CatalogVersionRef) => void;
  onClose: () => void;
}

/**
 * Drafts a specimen validity policy version or edits an open draft (REQ-SPC-008): the
 * categories a validity assessment can reach, each mapped to the action the laboratory
 * approved. Categories and actions come from the laboratory's policy, never from the app.
 */
export function ValidityDraftDialog({
  specimenTypeId,
  mode,
  onSaved,
  onClose,
}: ValidityDraftDialogProps) {
  const base = draftBase(mode);
  const form = useForm<ValidityFormValues>({
    defaultValues: { categories: base ? base.categories.map(categoryValues) : [EMPTY_CATEGORY] },
  });
  return (
    <DraftDialog<
      ValidityFormValues,
      ValidityPolicyView,
      DraftValidityPolicyVersion,
      UpdateValidityPolicyDraft
    >
      mode={mode}
      commands={{ draft: "draft_validity_policy_version", update: "update_validity_policy_draft" }}
      invalidates={VALIDITY_POLICY_QUERIES}
      form={form}
      newInput={(values) => ({ specimenTypeId, content: toContent(values) })}
      editInput={(draft, values) => ({
        policyVersionId: draft.id,
        expectedRevision: draft.revision,
        content: toContent(values),
      })}
      onSaved={onSaved}
      onClose={onClose}
    >
      <CategoryList />
    </DraftDialog>
  );
}

function CategoryList() {
  const { control } = useFormContext<ValidityFormValues>();
  const rows = useFieldArray({ control, name: "categories" });
  return (
    <FieldList
      name="categories"
      legend="Categories"
      emptyText="Add at least one category."
      items={rows.fields}
      itemLabel={(index) => `Category ${index + 1}`}
      addLabel="Add category"
      onAdd={() => rows.append(EMPTY_CATEGORY)}
      onRemove={rows.remove}
      renderItem={(index) => <CategoryFields index={index} />}
    />
  );
}

function CategoryFields({ index }: { index: number }) {
  const action = useWatch<ValidityFormValues, `categories.${number}.action`>({
    name: `categories.${index}.action`,
  });
  return (
    <>
      <TextInput name={`categories.${index}.code`} label="Code" isRequired />
      <TextInput name={`categories.${index}.label`} label="Label" isRequired />
      <ChoiceInput
        name={`categories.${index}.action`}
        label="Action"
        options={VALIDITY_ACTIONS}
        isRequired
      />
      {action === "qualifier" && (
        <TextInput
          name={`categories.${index}.qualifierText`}
          label="Qualifier text"
          description="Shown on the report, exactly as the policy words it."
          isRequired
        />
      )}
    </>
  );
}

function categoryValues(category: ValidityPolicyView["categories"][number]): CategoryValues {
  return {
    code: category.code,
    label: category.label,
    action: category.action,
    qualifierText: category.qualifierText ?? "",
  };
}

function toContent(values: ValidityFormValues): ValidityPolicyContent {
  return {
    categories: values.categories.map((category) => {
      const action = checked(choiceOf(VALIDITY_ACTIONS, category.action));
      return {
        code: category.code.trim(),
        label: category.label.trim(),
        action,
        // Only the qualifier action carries text; the field is hidden otherwise.
        qualifierText: action === "qualifier" ? category.qualifierText.trim() : null,
      };
    }),
  };
}
