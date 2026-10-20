import { useContext, useRef } from "react";
import { FormValidationContext } from "react-aria-components";
import {
  useController,
  useFieldArray,
  useFormContext,
  useFormState,
  useWatch,
} from "react-hook-form";
import { Button } from "@/components/Button";
import { ChoiceInput } from "@/components/FormFields";
import { Inset } from "@/components/Inset";
import { Section } from "@/components/Section";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import styles from "../accessioning.module.css";
import { REFLEX_AUTHORIZATIONS, SEQUENCE_ROLES } from "./labels";
import formStyles from "./OrderForm.module.css";
import type { OrderFormValues } from "./orderValues";
import { orderableLabel } from "./references";
import { TestChecklist } from "./TestChecklist";

/**
 * The tests marked on the requisition, from the current approved catalog, with the testing
 * sequence role and reflex authority each one carries (REQ-ORD-001, REQ-ORD-003,
 * REQ-RES-002). Nothing is preselected.
 */
export function TestSelection({ orderables }: { orderables: readonly OrderableVersionView[] }) {
  const { control } = useFormContext<OrderFormValues>();
  const selection = useRef<HTMLDivElement>(null);
  const items = useFieldArray({
    control,
    name: "items",
    rules: { validate: (value) => value.length > 0 || "Choose at least one test." },
  });
  const selected = useWatch({ control, name: "items" });
  const { errors } = useFormState({ control, name: "items" });

  // A range selection can change several IDs. Retain supplied fields on every surviving item.
  function change(next: string[]) {
    const current = selected.map((item) => item.orderableVersionId);
    const added = next.filter((id) => !current.includes(id)).map(emptyItem);
    const removed = current.flatMap((id, index) => (next.includes(id) ? [] : [index]));
    if (added.length && removed.length) {
      items.replace(
        next.map((id) => selected.find((item) => item.orderableVersionId === id) ?? emptyItem(id)),
      );
    } else if (added.length) items.append(added, { shouldFocus: false });
    else if (removed.length) items.remove(removed);
  }

  return (
    <Section title="Tests and panels">
      <div ref={selection} className={styles.group}>
        <TestChecklist
          legend="Tests ordered"
          name="tests"
          orderables={orderables}
          idOf={versionId}
          chosen={selected.map((item) => item.orderableVersionId)}
          onChange={change}
          isRequired
          errorMessage={errors.items?.root?.message}
        >
          <ul className={styles.list} aria-label="Chosen tests">
            {items.fields.length === 0 && <li className={styles.hint}>No tests chosen yet.</li>}
            {items.fields.map((field, index) => {
              const orderable = orderables.find((each) => each.id === field.orderableVersionId);
              return (
                <li key={field.id}>
                  <ReflexFields
                    index={index}
                    title={orderable ? orderableLabel(orderable) : "Test"}
                    others={orderables.filter((each) => each.id !== field.orderableVersionId)}
                    onRemove={() => {
                      items.remove(index);
                      selection.current?.querySelector<HTMLInputElement>("input")?.focus();
                    }}
                  />
                </li>
              );
            })}
          </ul>
        </TestChecklist>
      </div>
    </Section>
  );
}

function emptyItem(orderableVersionId: string): OrderFormValues["items"][number] {
  return {
    orderableVersionId,
    sequenceRole: null,
    reflexAuthorization: null,
    reflexScopeOrderableIds: [],
  };
}

interface ReflexFieldsProps {
  index: number;
  title: string;
  /** The other orderables a reflex may extend to. */
  others: readonly OrderableVersionView[];
}

function ReflexFields({
  index,
  title,
  others,
  onRemove,
}: ReflexFieldsProps & { onRemove: () => void }) {
  const { control } = useFormContext<OrderFormValues>();
  const authorization = useWatch({ control, name: `items.${index}.reflexAuthorization` });
  return (
    <Inset as="fieldset">
      <legend className={styles.legend}>{title}</legend>
      <div className={formStyles.selectedFields}>
        <ChoiceInput
          name={`items.${index}.sequenceRole`}
          label={`Testing sequence role for ${title}`}
          options={SEQUENCE_ROLES}
          description="As the requisition states it."
          isRequired
        />
        <ChoiceInput
          name={`items.${index}.reflexAuthorization`}
          label={`Reflex testing for ${title}`}
          options={REFLEX_AUTHORIZATIONS}
          description="As the requisition states it."
          isRequired
        />
        <Button variant="quiet" aria-label={`Remove ${title}`} onPress={onRemove}>
          Remove
        </Button>
      </div>
      {authorization === "authorized" && (
        <ReflexScope index={index} title={title} others={others} />
      )}
    </Inset>
  );
}

const versionId = (orderable: OrderableVersionView) => orderable.id;
const orderableId = (orderable: OrderableVersionView) => orderable.orderableId;

/**
 * The tests a reflex may extend to, by stable orderable ID, found like the tests ordered.
 * The engine's error for the scope shows under it.
 */
function ReflexScope({ index, title, others }: ReflexFieldsProps) {
  const { control } = useFormContext<OrderFormValues>();
  const { field, fieldState } = useController({
    control,
    name: `items.${index}.reflexScopeOrderableIds`,
  });
  const engineError = useContext(FormValidationContext)[field.name];
  const engineMessage = Array.isArray(engineError) ? engineError.join(" ") : engineError;
  return (
    <TestChecklist
      legend="Reflex tests authorized"
      name={`reflex tests for ${title}`}
      orderables={others}
      idOf={orderableId}
      chosen={field.value}
      onChange={field.onChange}
      errorMessage={fieldState.error?.message ?? engineMessage}
    />
  );
}
