import { Checkbox } from "@/components/Checkbox";
import layout from "@/styles/layout.module.css";
import styles from "./starter.module.css";

interface ItemCheckProps {
  name: string;
  code: string;
  /** Why it is checked or cannot be chosen, such as "Needed by Urine opioids". */
  note: string | null;
  isSelected: boolean;
  isDisabled: boolean;
  onChange: (isSelected: boolean) => void;
  /** Marks an analyte removed from its method. */
  removed?: boolean;
}

/**
 * One Standard Catalog item with its checkbox: name and code as configured in the file, and
 * the note beside it, outside the checkbox, so a disabled item's reason stays readable.
 */
export function ItemCheck({ name, code, note, removed = false, ...check }: ItemCheckProps) {
  return (
    <div className={styles.itemRow}>
      <Checkbox {...check}>
        <span className={removed ? styles.removed : undefined}>
          {name} <span className={layout.code}>{code}</span>
        </span>
      </Checkbox>
      {note && <span className={styles.note}>{note}</span>}
    </div>
  );
}
