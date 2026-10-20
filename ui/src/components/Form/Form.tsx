import { type FormEvent, type ReactNode, useEffect, useRef } from "react";
import { Form as AriaForm } from "react-aria-components";
import { ErrorState } from "@/components/ErrorState";
import type { CommandError } from "@/generated/CommandError";
import { type FieldPaths, toValidationErrors } from "@/lib/errors";
import styles from "./Form.module.css";

interface FormProps {
  children: ReactNode;
  /** Buttons shown below the fields, primary action first. */
  actions: ReactNode;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  /**
   * The engine's answer to the last submit. Its field errors show on the fields with the
   * matching `name`; its message and support reference show above the fields.
   */
  error?: CommandError | null;
  /** Heading of the error message, such as "Sign-in failed". */
  errorTitle?: string;
  /** Where the form's field names differ from the command input's (see `toFormField`). */
  fieldPaths?: FieldPaths;
  /** Keep long task-form actions available while scrolling within the form. */
  stickyActions?: boolean;
  "aria-label"?: string;
}

/**
 * Vertical form layout. Pair with react-hook-form and the field components. When the engine
 * reports field errors, focus moves to the first invalid field.
 */
export function Form({
  children,
  actions,
  error = null,
  errorTitle,
  fieldPaths,
  stickyActions = false,
  ...rest
}: FormProps) {
  const formRef = useRef<HTMLFormElement>(null);
  // Moves DOM focus, which lives outside React, once the errors are on the fields.
  useEffect(() => {
    if (!error?.fieldErrors.length) return;
    formRef.current?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
  }, [error]);
  return (
    <AriaForm
      {...rest}
      ref={formRef}
      validationBehavior="aria"
      validationErrors={toValidationErrors(error, fieldPaths)}
      className={styles.form}
      data-sticky-actions={stickyActions || undefined}
    >
      {error && <ErrorState title={errorTitle ?? "The action failed"} error={error} />}
      {children}
      <div className={styles.actions}>{actions}</div>
    </AriaForm>
  );
}
