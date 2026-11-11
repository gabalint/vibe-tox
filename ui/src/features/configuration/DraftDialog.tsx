import type { ReactNode } from "react";
import type { FieldValues, UseFormReturn } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { useToast } from "@/components/Toast";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";

/**
 * A new draft, empty or copied from an existing version's content, or an edit of an open
 * draft. Only the draft's author may edit it; the engine refuses anyone else.
 */
export type DraftMode<View> = { kind: "new"; base: View | null } | { kind: "edit"; draft: View };

interface DraftDialogProps<Values extends FieldValues, View, NewInput, EditInput> {
  mode: DraftMode<View>;
  /** `draft_*_version` for a new draft, `update_*_draft` for an edit. */
  commands: { draft: CommandName; update: CommandName };
  invalidates: readonly CommandName[];
  form: UseFormReturn<Values>;
  newInput: (values: Values) => NewInput;
  editInput: (draft: View, values: Values) => EditInput;
  onSaved: (draft: CatalogVersionRef) => void;
  onClose: () => void;
  /** `wide` for long drafts grouped in `FieldSection`s. */
  size?: "default" | "wide";
  children: ReactNode;
}

/** Saves a draft version's content with one command, then reports the saved draft. */
export function DraftDialog<
  Values extends FieldValues,
  View extends { version: number },
  NewInput,
  EditInput,
>({
  mode,
  commands,
  invalidates,
  form,
  newInput,
  editInput,
  onSaved,
  onClose,
  size,
  children,
}: DraftDialogProps<Values, View, NewInput, EditInput>) {
  const toast = useToast();
  const save = useCommandMutation<CatalogVersionRef, { input: NewInput | EditInput }>(
    mode.kind === "edit" ? commands.update : commands.draft,
    { invalidates: [...invalidates] },
  );
  const onSubmit = form.handleSubmit((values) =>
    save.mutate(
      { input: mode.kind === "edit" ? editInput(mode.draft, values) : newInput(values) },
      {
        onSuccess: (draft) => {
          toast.show({
            tone: "success",
            title: `Draft version ${draft.version} saved`,
            description:
              "It is approved before it is used, by someone other than its author unless your laboratory allows self-approval.",
          });
          onSaved(draft);
          onClose();
        },
      },
    ),
  );
  return (
    <FormDialog
      title={draftTitle(mode)}
      submitLabel="Save draft"
      form={form}
      onSubmit={onSubmit}
      command={save}
      errorTitle="The draft was not saved"
      onClose={onClose}
      size={size}
    >
      {children}
    </FormDialog>
  );
}

function draftTitle(mode: DraftMode<{ version: number }>): string {
  if (mode.kind === "edit") return `Edit draft version ${mode.draft.version}`;
  return mode.base ? `New draft from version ${mode.base.version}` : "New draft version";
}

/** The version a draft starts from: the draft itself when editing. */
export function draftBase<View>(mode: DraftMode<View>): View | null {
  return mode.kind === "edit" ? mode.draft : mode.base;
}
