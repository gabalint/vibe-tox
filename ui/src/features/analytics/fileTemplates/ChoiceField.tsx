import { ChoiceInput, RecordPickerInput } from "@/components/FormFields";
import type { ColumnOption } from "./header";

/** Up to this many options show as a drop-down; more are found by typing (UI standards). */
const SELECT_UP_TO = 15;

interface ChoiceFieldProps {
  /** The form path; also the key for engine field errors. */
  name: string;
  label: string;
  options: readonly ColumnOption[];
  isRequired?: boolean;
  description?: string;
  onChange?: (value: string | null) => void;
}

/**
 * One choice among a sample's columns or lines: a drop-down for a short list, and a field
 * that narrows by typing for a long one. Each option keeps its example value.
 */
export function ChoiceField({ options, ...field }: ChoiceFieldProps) {
  if (options.length > SELECT_UP_TO) return <RecordPickerInput {...field} options={options} />;
  const labelled = options.map(({ id, label, description }) => ({
    id,
    label: description === undefined ? label : `${label} (${description})`,
  }));
  return <ChoiceInput {...field} options={labelled} />;
}
