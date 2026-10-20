import { type ReactNode, useMemo, useRef, useState } from "react";
import { Button } from "@/components/Button";
import { CheckboxGroup } from "@/components/CheckboxGroup";
import { RecordPicker } from "@/components/RecordPicker";
import { Select } from "@/components/Select";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import styles from "../accessioning.module.css";
import formStyles from "./OrderForm.module.css";
import { testChoiceLabel } from "./references";

const CHECKLIST_LIMIT = 15;
const CATALOG_KINDS = [
  { id: "all", label: "Tests and panels" },
  { id: "test", label: "Tests only" },
  { id: "panel", label: "Panels only" },
];

interface TestChecklistProps {
  legend: string;
  name: string;
  orderables: readonly OrderableVersionView[];
  /** Version ID for ordered tests; stable orderable ID for reflex scope. */
  idOf: (orderable: OrderableVersionView) => string;
  chosen: readonly string[];
  onChange: (chosen: string[]) => void;
  isRequired?: boolean;
  errorMessage: string | undefined;
  /** Ordered items supply their required per-item fields in place of the simple chosen list. */
  children?: ReactNode;
}

/** Long catalogs use the shared virtualized picker; filtering never removes a chosen item. */
export function TestChecklist({
  legend,
  name,
  orderables,
  idOf,
  children,
  ...choice
}: TestChecklistProps) {
  const [kind, setKind] = useState<string | null>("all");
  const inputRef = useRef<HTMLInputElement>(null);
  const findable = orderables.length > CHECKLIST_LIMIT;
  const options = useMemo(
    () =>
      orderables
        .filter((each) => kind === "all" || each.kind === kind)
        .map((each) => ({
          id: idOf(each),
          label: testChoiceLabel(each),
          description: each.kind === "panel" ? "Panel" : "Test",
        })),
    [orderables, idOf, kind],
  );
  return (
    <>
      {findable ? (
        <div className={formStyles.catalogControls}>
          <RecordPicker
            label={`Find ${name}`}
            inputRef={inputRef}
            name={name}
            selectionMode="multiple"
            options={options}
            value={choice.chosen}
            onChange={choice.onChange}
            isRequired={choice.isRequired}
            errorMessage={choice.errorMessage}
            description="Search by code, name or specimen type."
          />
          <Select
            label="Show"
            name={`${name}-kind`}
            options={CATALOG_KINDS}
            value={kind}
            onChange={setKind}
          />
        </div>
      ) : (
        <CheckboxGroup
          legend={legend}
          options={options}
          value={choice.chosen}
          onChange={choice.onChange}
          isRequired={choice.isRequired}
          errorMessage={choice.errorMessage}
        />
      )}
      <p className={styles.hint} role="status">
        {choice.chosen.length} selected
      </p>
      {children ??
        (findable && (
          <ul className={styles.list} aria-label={`Chosen ${name}`}>
            {choice.chosen.length === 0 && <li className={styles.hint}>No tests chosen yet.</li>}
            {choice.chosen.map((id) => {
              const orderable = orderables.find((each) => idOf(each) === id);
              const label = orderable ? testChoiceLabel(orderable) : "Test";
              return (
                <li key={id} className={styles.listRow}>
                  {label}
                  <Button
                    variant="quiet"
                    aria-label={`Remove ${label}`}
                    onPress={() => {
                      choice.onChange(choice.chosen.filter((each) => each !== id));
                      inputRef.current?.focus();
                    }}
                  >
                    Remove
                  </Button>
                </li>
              );
            })}
          </ul>
        ))}
    </>
  );
}
