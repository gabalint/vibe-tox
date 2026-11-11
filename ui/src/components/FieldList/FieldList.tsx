import { type ReactNode, useContext, useId } from "react";
import { FormValidationContext } from "react-aria-components";
import { Button } from "@/components/Button";
import { FieldGrid } from "@/components/Form";
import { Inset } from "@/components/Inset";
import styles from "./FieldList.module.css";

interface FieldListProps {
  /** The form path of the list, such as `diagnoses`. */
  name: string;
  legend: string;
  /** Shown when the list is empty, such as "No diagnoses on the requisition." */
  emptyText: string;
  /** The rows, with react-hook-form's `useFieldArray` keys. */
  items: readonly { id: string }[];
  /** The legend of one row, such as "Diagnosis 2". */
  itemLabel: (index: number) => string;
  addLabel: string;
  onAdd: () => void;
  onRemove: (index: number) => void;
  renderItem: (index: number) => ReactNode;
}

/**
 * A repeatable group of fields: as many rows as the requisition supplies, none assumed. The
 * engine's error for the whole list, such as "Add at least one analyte.", shows under the
 * legend as the surrounding `Form` names it, and the list takes the form's error focus.
 */
export function FieldList({
  name,
  legend,
  emptyText,
  items,
  itemLabel,
  addLabel,
  onAdd,
  onRemove,
  renderItem,
}: FieldListProps) {
  const errorId = useId();
  const engineError = useContext(FormValidationContext)[name];
  const errorMessage = Array.isArray(engineError) ? engineError.join(" ") : engineError;
  return (
    <fieldset
      className={styles.list}
      aria-describedby={errorMessage ? errorId : undefined}
      aria-invalid={errorMessage ? true : undefined}
      tabIndex={errorMessage ? -1 : undefined}
    >
      <legend className={styles.legend}>{legend}</legend>
      {errorMessage && (
        <p id={errorId} className={styles.error}>
          {errorMessage}
        </p>
      )}
      {items.length === 0 && <p className={styles.empty}>{emptyText}</p>}
      {items.map((item, index) => (
        <Inset key={item.id} as="fieldset">
          <legend className={styles.legend}>{itemLabel(index)}</legend>
          <FieldGrid>{renderItem(index)}</FieldGrid>
          <div>
            <Button variant="quiet" onPress={() => onRemove(index)}>
              Remove {itemLabel(index).toLowerCase()}
            </Button>
          </div>
        </Inset>
      ))}
      <div>
        <Button onPress={onAdd}>{addLabel}</Button>
      </div>
    </fieldset>
  );
}
