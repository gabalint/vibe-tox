import type { FormEvent, ReactNode } from "react";
import { Dialog as AriaDialog, Heading, Modal, ModalOverlay } from "react-aria-components";
import styles from "./Dialog.module.css";

interface DialogProps {
  title: string;
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  children: ReactNode;
  /** Buttons in the footer; omit when the content is a `Form` with its own actions. */
  actions?: ReactNode;
  /** `alertdialog` for confirmations that interrupt the user. */
  role?: "dialog" | "alertdialog";
  /** Closes on outside clicks, and on Escape unless `isKeyboardDismissDisabled`. */
  isDismissable?: boolean;
  /** Escape does not close the dialog. Defaults to the opposite of `isDismissable`. */
  isKeyboardDismissDisabled?: boolean;
  /**
   * Makes the content and the actions one form: Enter in a field, or a `submit` button among
   * the actions, calls it.
   */
  onSubmit?: () => void;
}

/** A modal dialog: traps focus while open and returns it to the trigger when closed. */
export function Dialog({
  title,
  isOpen,
  onOpenChange,
  children,
  actions,
  role = "dialog",
  isDismissable = true,
  isKeyboardDismissDisabled = !isDismissable,
  onSubmit,
}: DialogProps) {
  const content = (
    <>
      <div className={styles.body}>{children}</div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </>
  );
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // The dialog renders in a portal; keep its submit from reaching a form around the trigger.
    event.stopPropagation();
    onSubmit?.();
  }
  return (
    <ModalOverlay
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      isDismissable={isDismissable}
      isKeyboardDismissDisabled={isKeyboardDismissDisabled}
      className={styles.overlay}
    >
      <Modal className={styles.modal}>
        <AriaDialog role={role} className={styles.dialog}>
          <Heading slot="title" className={styles.title}>
            {title}
          </Heading>
          {onSubmit ? (
            <form onSubmit={submit} className={styles.form}>
              {content}
            </form>
          ) : (
            content
          )}
        </AriaDialog>
      </Modal>
    </ModalOverlay>
  );
}
