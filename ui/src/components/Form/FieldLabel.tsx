import { Label } from "react-aria-components";
import styles from "./Form.module.css";

interface FieldLabelProps {
  children: string;
  isRequired?: boolean;
}

/** A field's visible label, with the required marker. */
export function FieldLabel({ children, isRequired }: FieldLabelProps) {
  return (
    <Label className={styles.label}>
      {children}
      {isRequired && (
        <span className={styles.required} aria-hidden="true">
          {" "}
          *
        </span>
      )}
    </Label>
  );
}
