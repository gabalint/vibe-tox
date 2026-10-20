import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { FieldGrid } from "@/components/Form";
import { Inset } from "@/components/Inset";
import styles from "./FieldList.module.css";

interface FieldListProps {
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

/** A repeatable group of fields: as many rows as the requisition supplies, none assumed. */
export function FieldList({
  legend,
  emptyText,
  items,
  itemLabel,
  addLabel,
  onAdd,
  onRemove,
  renderItem,
}: FieldListProps) {
  return (
    <fieldset className={styles.list}>
      <legend className={styles.legend}>{legend}</legend>
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
