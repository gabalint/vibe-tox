import { type ReactNode, useState } from "react";
import { type DefaultValues, type FieldValues, useForm } from "react-hook-form";
import { FormDialog } from "@/components/FormDialog";
import { asksSelfApproval, SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import { useToast } from "@/components/Toast";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";
import type { FieldPaths } from "@/lib/errors";

interface CommandDialogProps<Values extends FieldValues, Input, Result> {
  title: string;
  submitLabel: string;
  /** The engine command, run with `{ input }`, and the queries it makes stale. */
  command: { name: CommandName; invalidates: readonly CommandName[] };
  defaultValues: DefaultValues<Values>;
  /**
   * The command's input. `selfApproval` is whether the person confirmed doing both steps of a
   * duty alone (ADR 0015): `false` until the engine asks for that confirmation and it is given.
   */
  toInput: (values: Values, selfApproval: boolean) => Input;
  /** The notice once the engine has recorded the action, or how to word it from the result. */
  successTitle: string | ((values: Values, result: Result) => string);
  /** The title over the engine's refusal; "Not recorded" by default. */
  errorTitle?: string;
  onDone?: (result: Result) => void;
  onClose: () => void;
  /** Where engine field errors land when the input's shape differs from the form's. */
  fieldPaths?: FieldPaths;
  /** The fields, bound to the form by name. */
  children: ReactNode;
}

/**
 * A dialog whose form runs one engine command. Nothing is preselected; the engine validates
 * every field again and its field errors show on the matching fields; a request to confirm a
 * self-approval shows that confirmation. Mount it only while open. It lives in `lib/` rather
 * than `components/` because it calls the engine, and several features use it.
 */
export function CommandDialog<Values extends FieldValues, Input, Result = unknown>(
  props: CommandDialogProps<Values, Input, Result>,
) {
  const { title, submitLabel, command, defaultValues, toInput, successTitle, onDone, onClose } =
    props;
  const toast = useToast();
  const [selfApproval, setSelfApproval] = useState(false);
  const run = useCommandMutation<Result, { input: Input }>(command.name, {
    invalidates: command.invalidates,
  });
  const form = useForm<Values>({ defaultValues });
  const submit = form.handleSubmit((values) => {
    const onSuccess = (result: Result) => {
      const notice = typeof successTitle === "string" ? successTitle : successTitle(values, result);
      toast.show({ tone: "success", title: notice });
      onDone?.(result);
      onClose();
    };
    run.mutate({ input: toInput(values, selfApproval) }, { onSuccess });
  });
  return (
    <FormDialog
      form={form}
      title={title}
      submitLabel={submitLabel}
      errorTitle={props.errorTitle ?? "Not recorded"}
      fieldPaths={props.fieldPaths}
      // A request to confirm a self-approval is a question, shown below the fields.
      command={{ error: asksSelfApproval(run.error) ? null : run.error, isPending: run.isPending }}
      onSubmit={submit}
      onClose={onClose}
    >
      {props.children}
      <SelfApprovalConfirm
        error={run.error}
        isConfirmed={selfApproval}
        onChange={setSelfApproval}
      />
    </FormDialog>
  );
}
