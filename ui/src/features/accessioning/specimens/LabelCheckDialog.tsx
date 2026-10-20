import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { TextField } from "@/components/TextField";

interface LabelCheckDialogProps {
  /** The title of the action the check leads to, such as "Create aliquot from C00000001". */
  title: string;
  /** What a matching label is, for the mismatch message: "a container of Specimen 1 (urine)". */
  expected: string;
  /** The label codes the engine sent for the containers that may be scanned. */
  labelCodes: readonly string[];
  /** The scanned or typed label matched one of `labelCodes`; the action's own form opens. */
  onConfirmed: (labelCode: string) => void;
  onClose: () => void;
}

/**
 * Confirms the container in hand by scanning or typing its label, never by pressing OK
 * (ui-standards, Clinical display rules). A scanner types the label and presses Enter. The
 * entry must equal a label code the engine sent, apart from surrounding spaces. This is a
 * presentation guard only: the engine's commands do not receive the scanned label yet.
 */
export function LabelCheckDialog({
  title,
  expected,
  labelCodes,
  onConfirmed,
  onClose,
}: LabelCheckDialogProps) {
  const [entry, setEntry] = useState("");
  const [mismatch, setMismatch] = useState<string | null>(null);
  const check = () => {
    const scanned = entry.trim();
    if (labelCodes.includes(scanned)) {
      onConfirmed(scanned);
      return;
    }
    setMismatch(
      scanned === ""
        ? "Scan or type the container label."
        : `${scanned} is not ${expected}. Check the container in your hand.`,
    );
    setEntry("");
  };
  return (
    <Dialog
      title={title}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
      onSubmit={check}
      actions={
        <>
          <Button type="submit" variant="primary">
            Check label
          </Button>
          <Button onPress={onClose}>Cancel</Button>
        </>
      }
    >
      <TextField
        label="Container label"
        name="labelCode"
        value={entry}
        onChange={(value) => {
          setEntry(value);
          setMismatch(null);
        }}
        description="Scan the label on the container, or type it."
        errorMessage={mismatch ?? undefined}
        isRequired
        autoFocus
        autoComplete="off"
      />
    </Dialog>
  );
}
