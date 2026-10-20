import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Checkbox } from "@/components/Checkbox";
import type { CommandError } from "@/generated/CommandError";
import styles from "./SelfApprovalConfirm.module.css";

/** The input field on which the engine asks the person to confirm a self-approval (ADR 0015). */
const FIELD = "selfApproval";

/** The engine's request to confirm a self-approval, or `null` when `error` is something else. */
function askOf(error: CommandError | null): string | null {
  if (error?.code !== "validation") return null;
  return error.fieldErrors.find((each) => each.field === FIELD)?.message ?? null;
}

/**
 * Whether `error` only asks the person to confirm a self-approval. Such an answer is a question,
 * not a refusal: dialogs show `SelfApprovalConfirm` for it instead of their error message.
 */
export function asksSelfApproval(error: CommandError | null): boolean {
  return askOf(error) !== null;
}

interface SelfApprovalConfirmProps {
  /** The command's last answer. The confirmation appears once it asks for one. */
  error: CommandError | null;
  isConfirmed: boolean;
  onChange: (isConfirmed: boolean) => void;
}

/**
 * The confirmation a person gives to do both steps of a duty alone, when the laboratory's policy
 * allows it and the engine asks for it: the engine's words, and an unticked checkbox that takes
 * focus. Once asked it stays until the dialog closes, so the person can tick it and confirm again.
 * Mount it inside a dialog that is mounted only while open, so nothing carries over.
 */
export function SelfApprovalConfirm({ error, isConfirmed, onChange }: SelfApprovalConfirmProps) {
  const ask = askOf(error);
  const [message, setMessage] = useState<string | null>(null);
  // Keeps the latest request while later answers, such as a wrong password, replace the error.
  if (ask !== null && ask !== message) setMessage(ask);
  const shown = ask ?? message;
  if (shown === null) return null;
  return (
    <div className={styles.confirm}>
      <Alert tone="warning" title="Confirm self-approval">
        <p>{shown}</p>
      </Alert>
      <Checkbox name={FIELD} isSelected={isConfirmed} onChange={onChange} autoFocus>
        I confirm that no second person reviewed this
      </Checkbox>
    </div>
  );
}
