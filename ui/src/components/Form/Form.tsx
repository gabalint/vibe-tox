import { type FormEvent, type ReactNode, useEffect, useRef, useState } from "react";
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

const NO_NAMES: ReadonlySet<string> = new Set();

/**
 * Vertical form layout. Pair with react-hook-form and the field components. When the engine
 * reports field errors, focus moves to the first invalid field, and a field's error goes away
 * as soon as its value is edited.
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
  // Fields edited since the engine's last answer. React Aria clears an engine error only on the
  // input's change event, which a text box fires on blur, so typing a fix left it showing.
  const [edited, setEdited] = useState({ error, names: NO_NAMES });
  const editedNames = edited.error === error ? edited.names : NO_NAMES;
  const validationErrors = toValidationErrors(error, fieldPaths);
  for (const name of editedNames) delete validationErrors[name];
  useEffect(() => {
    const form = formRef.current;
    if (!form) return;
    const onInput = (event: Event) => {
      const { name } = event.target as HTMLInputElement;
      if (!name) return;
      setEdited((last) => {
        const names = last.error === error ? last.names : NO_NAMES;
        return names.has(name) ? last : { error, names: new Set(names).add(name) };
      });
    };
    form.addEventListener("input", onInput);
    return () => form.removeEventListener("input", onInput);
  }, [error]);
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
      validationErrors={validationErrors}
      className={styles.form}
      data-sticky-actions={stickyActions || undefined}
    >
      {error && <ErrorState title={errorTitle ?? "The action failed"} error={error} />}
      {children}
      <div className={styles.actions}>{actions}</div>
    </AriaForm>
  );
}
