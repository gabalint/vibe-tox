import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { FieldList } from "@/components/FieldList";
import { Form } from "@/components/Form";
import { CheckList, ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Section } from "@/components/Section";
import { SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import { TextField } from "@/components/TextField";
import type { CommandError } from "@/generated/CommandError";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { GallerySection } from "./GallerySection";

/** The engine's request to confirm a self-approval (ADR 0015), with synthetic wording. */
const SELF_APPROVAL_ASK: CommandError = {
  code: "validation",
  message: "Check the highlighted fields.",
  fieldErrors: [
    {
      field: "selfApproval",
      code: "self_approval_unconfirmed",
      message: "You wrote this synthetic draft. Confirm that no second person reviewed it.",
    },
  ],
  gateReasons: [],
  correlationId: "corr-0000-synthetic",
};

/** The engine's refusal of an empty repeatable list, with synthetic wording. */
const EMPTY_LIST_REFUSAL: CommandError = {
  code: "validation",
  message: "Check the highlighted fields.",
  fieldErrors: [{ field: "rows", code: "required", message: "Add at least one synthetic row." }],
  gateReasons: [],
  correlationId: "corr-0000-synthetic",
};

const MATRICES = [
  { id: "urine", label: "Urine" },
  { id: "oral_fluid", label: "Oral fluid" },
];

/** The form-bound fields, repeatable field groups, titled sections and reason confirmation. */
export function FormPartsSection() {
  const form = useForm({ defaultValues: { code: "", date: "", matrix: null, panels: [] } });
  const [rows, setRows] = useState<{ id: string }[]>([{ id: "row-0" }]);
  const [listRefused, setListRefused] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [draft, setDraft] = useState("");
  const [asked, setAsked] = useState(false);
  const [selfApproved, setSelfApproved] = useState(false);
  return (
    <GallerySection title="Form parts and sections">
      <Section
        title="Synthetic record"
        actions={<Button onPress={() => setConfirming(true)}>Retire</Button>}
      >
        <FormProvider {...form}>
          <Form
            aria-label="Form part examples"
            stickyActions
            onSubmit={form.handleSubmit(() => undefined)}
            error={listRefused ? EMPTY_LIST_REFUSAL : null}
            errorTitle="The synthetic record was not saved"
            actions={
              <>
                <Button type="submit" variant="primary">
                  Check fields
                </Button>
                <Button onPress={() => setListRefused((refused) => !refused)}>
                  {listRefused ? "Clear list error" : "Show list error"}
                </Button>
              </>
            }
          >
            <TextInput name="code" label="Code" isRequired />
            <TextInput name="date" label="Date" format={DATE_FORMAT} />
            <ChoiceInput name="matrix" label="Matrix" options={MATRICES} isRequired />
            <CheckList name="panels" legend="Panels" options={MATRICES} />
            <FieldList
              name="rows"
              legend="Repeatable rows"
              emptyText="No rows."
              items={rows}
              itemLabel={(index) => `Row ${index + 1}`}
              addLabel="Add row"
              onAdd={() => setRows((all) => [...all, { id: `row-${all.length}` }])}
              onRemove={(index) => setRows((all) => all.filter((_, each) => each !== index))}
              renderItem={(index) => <p>Fields of row {index + 1}</p>}
            />
          </Form>
        </FormProvider>
      </Section>
      <Section title="Unsaved changes" variant="plain">
        <TextField label="Synthetic draft note" name="draft" value={draft} onChange={setDraft} />
        <p>
          Once something is typed, <Link to="/">leaving the gallery</Link> asks first.
        </p>
        <UnsavedChangesGuard when={draft !== ""} />
      </Section>
      <Section
        title="Self-approval confirmation"
        actions={<Button onPress={() => setAsked(true)}>Ask for confirmation</Button>}
      >
        <SelfApprovalConfirm
          error={asked ? SELF_APPROVAL_ASK : null}
          isConfirmed={selfApproved}
          onChange={setSelfApproved}
        />
      </Section>
      {confirming && (
        <ReasonConfirmDialog
          title="Retire the synthetic record?"
          confirmLabel="Retire"
          variant="danger"
          command={{ mutate: () => setConfirming(false), error: null, isPending: false }}
          input={(reason) => ({ reason })}
          successTitle="Retired"
          onClose={() => setConfirming(false)}
        >
          <p>A reason is recorded with the change.</p>
        </ReasonConfirmDialog>
      )}
    </GallerySection>
  );
}
