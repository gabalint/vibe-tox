import { useFormContext, useWatch } from "react-hook-form";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { CheckList } from "@/components/FormFields";
import { Section } from "@/components/Section";
import type { OrderAccessionContext } from "@/generated/OrderAccessionContext";
import type { OrderDetail } from "@/generated/OrderDetail";
import { AccessionTestPicker } from "./AccessionTestPicker";
import type { AccessionFormValues } from "./accessionValues";
import styles from "./receiving.module.css";

interface Props {
  order: OrderDetail;
  context: OrderAccessionContext;
  selectable?: boolean;
}

/** Eligibility and its explanation come from the engine, including historical test names. */
export function AccessionTestSelection({ order, context, selectable = false }: Props) {
  const blocked = context.items.filter((item) => !item.eligible);
  const names = new Map(
    order.items.map((item) => [item.itemId, `${item.orderableCode} ${item.orderableName}`]),
  );
  return (
    <Section title="Ordered tests" variant="plain">
      <p className={styles.hint}>
        Order <Link to={`/orders/${order.orderId}`}>{order.placerOrderId ?? order.orderId}</Link>
      </p>
      {selectable && <EligibleTests order={order} context={context} />}
      {blocked.length > 0 && (
        <ul className={styles.list} aria-label="Tests unavailable for this accession">
          {blocked.map((item) => (
            <li key={item.orderItemId}>
              <strong>{names.get(item.orderItemId) ?? item.orderItemId}</strong>:{" "}
              {item.reason ?? "Unavailable for receiving."}
              {item.accessionId && (
                <>
                  {" "}
                  <Link to={`/accessions/${item.accessionId}`}>
                    Accession {item.accessionNumber ?? item.accessionId}
                  </Link>
                </>
              )}
            </li>
          ))}
        </ul>
      )}
      {context.accessions.length > 0 && (
        <ul className={styles.list} aria-label="Existing accessions">
          {context.accessions.map((accession) => (
            <li key={accession.accessionId}>
              <Link to={`/accessions/${accession.accessionId}`}>
                Accession {accession.accessionNumber}
              </Link>
              {` · ${accession.specimenCount} ${accession.specimenCount === 1 ? "specimen" : "specimens"} · ${accession.pendingReceiptCount} need receipt review`}
            </li>
          ))}
        </ul>
      )}
    </Section>
  );
}

function EligibleTests({ order, context }: Omit<Props, "selectable">) {
  const { control, setValue } = useFormContext<AccessionFormValues>();
  const selected = useWatch({ control, name: "orderItemIds" });
  const eligible = context.items.filter((item) => item.eligible);
  const names = new Map(
    order.items.map((item) => [item.itemId, `${item.orderableCode} ${item.orderableName}`]),
  );
  const required = [
    ...new Set(
      eligible
        .filter((item) => selected.includes(item.orderItemId))
        .flatMap((item) => item.requiredSpecimenTypeCodes),
    ),
  ];
  const options = eligible.map((item) => ({
    id: item.orderItemId,
    label: names.get(item.orderItemId) ?? item.orderItemId,
    description: `Specimen types: ${item.requiredSpecimenTypeCodes.join(", ")}`,
  }));
  return (
    <>
      <div className={styles.selectionActions}>
        <Button
          onPress={() =>
            setValue(
              "orderItemIds",
              eligible.map((item) => item.orderItemId),
              { shouldDirty: true, shouldValidate: true },
            )
          }
        >
          Use all eligible tests
        </Button>
        <Button
          variant="quiet"
          isDisabled={selected.length === 0}
          onPress={() => setValue("orderItemIds", [], { shouldDirty: true, shouldValidate: true })}
        >
          Clear selected tests
        </Button>
        <span className={styles.hint}>{selected.length} selected</span>
      </div>
      {options.length > 15 ? (
        <AccessionTestPicker options={options} />
      ) : (
        <CheckList
          name="orderItemIds"
          legend="Tests for this material"
          options={options.map(({ id, label, description }) => ({
            id,
            label,
            details: [description],
          }))}
          requiredMessage="Choose at least one test."
        />
      )}
      {required.length > 0 && (
        <p className={styles.hint} role="status">
          Required specimen types for the selected tests: <strong>{required.join(", ")}</strong>.
          Record each type received below.
        </p>
      )}
    </>
  );
}
