import { type ReactNode, useState } from "react";
import {
  type DefaultValues,
  type FieldValues,
  FormProvider,
  type Path,
  useForm,
} from "react-hook-form";
import { Alert } from "@/components/Alert";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { ErrorState } from "@/components/ErrorState";
import { ReasonField } from "@/components/ReasonField";
import { asksSelfApproval, SelfApprovalConfirm } from "@/components/SelfApprovalConfirm";
import { useToast } from "@/components/Toast";
import type { CommandName } from "@/generated/CommandName";
import { useCommandMutation } from "@/lib/commands";
import { AuthenticationField } from "@/lib/securityKeyAuthenticationField";
import styles from "@/styles/layout.module.css";

/** The fields every re-authenticated decision collects; `reason` only when asked for. */
export interface ReauthValues {
  password: string;
  reason: string;
}

interface ReauthConfirmDialogProps<Extra extends FieldValues, Input, Result> {
  title: string;
  confirmLabel: string;
  variant: "primary" | "danger";
  command: { name: CommandName; invalidates: readonly CommandName[] | "all" };
  /** Starting values of the further fields rendered as `children`. */
  extraDefaults: Extra;
  /** Adds the required reason field, recorded in the audit trail. */
  withReason?: boolean;
  /**
   * The command's input. `selfApproval` is whether the person confirmed doing both steps of a
   * duty alone (ADR 0015): `false` until the engine asks for that confirmation and it is given.
   */
  toInput: (values: Extra & ReauthValues, selfApproval: boolean) => Input;
  /** The notice once the engine has recorded the decision. */
  successTitle: (result: Result) => string;
  onDone?: (result: Result) => void;
  onClose: () => void;
  /** Heading of the engine's refusal, such as "Not exported". */
  errorTitle: string;
  /**
   * For commands that open an OS dialog in Rust and answer `null` when it is cancelled: what
   * the dialog says then. The dialog stays open, so the person can try again.
   */
  cancelledText?: string;
  /** What will happen, in laboratory terms, and any further fields. */
  children: ReactNode;
}

/**
 * Confirms a decision that needs the signed-in person's password again (REQ-IDN-004). Every
 * decision asks again; nothing is remembered. A wrong password and any other engine field error
 * show on their field, which takes focus, and every refusal stays in the dialog with its gate
 * reasons. A request to confirm a self-approval shows that confirmation instead, and the
 * password stays typed for the next attempt. Enter in the password field confirms. In `lib/`
 * because it calls the engine and several features use it.
 */
export function ReauthConfirmDialog<Extra extends FieldValues, Input, Result>({
  title,
  confirmLabel,
  variant,
  command,
  extraDefaults,
  withReason = false,
  toInput,
  successTitle,
  onDone,
  onClose,
  errorTitle,
  cancelledText,
  children,
}: ReauthConfirmDialogProps<Extra, Input, Result>) {
  const toast = useToast();
  const [cancelled, setCancelled] = useState(false);
  const [selfApproval, setSelfApproval] = useState(false);
  const run = useCommandMutation<Result, { input: Input }>(command.name, {
    invalidates: command.invalidates,
  });
  const form = useForm<Extra & ReauthValues>({
    defaultValues: { ...extraDefaults, password: "", reason: "" } as DefaultValues<
      Extra & ReauthValues
    >,
  });
  const confirm = form.handleSubmit((values) => {
    setCancelled(false);
    run.mutate(
      { input: toInput(values, selfApproval) },
      {
        onSuccess: (result) => {
          if (cancelledText !== undefined && result === null) {
            setCancelled(true);
            return;
          }
          toast.show({ tone: "success", title: successTitle(result) });
          onDone?.(result);
          onClose();
        },
        onError: (error) => {
          // Focus goes to the first field the engine refused, such as a wrong password.
          const fields = error.fieldErrors.filter((each) => Object.hasOwn(values, each.field));
          fields.forEach((each, index) => {
            form.setError(
              each.field as Path<Extra & ReauthValues>,
              { message: each.message },
              { shouldFocus: index === 0 },
            );
          });
        },
      },
    );
  });
  return (
    <ConfirmDialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      confirmLabel={confirmLabel}
      onConfirm={() => void confirm()}
      isPending={run.isPending}
      variant={variant}
    >
      <div className={styles.stack}>
        <FormProvider {...form}>
          {children}
          {withReason && <ReasonField />}
          <AuthenticationField />
        </FormProvider>
        <SelfApprovalConfirm
          error={run.error}
          isConfirmed={selfApproval}
          onChange={setSelfApproval}
        />
        {cancelled && cancelledText && <Alert tone="info">{cancelledText}</Alert>}
        {run.error && !asksSelfApproval(run.error) && (
          <ErrorState title={errorTitle} error={run.error} />
        )}
      </div>
    </ConfirmDialog>
  );
}
