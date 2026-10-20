import { Link } from "react-router";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { AccessionDetail } from "@/generated/AccessionDetail";
import type { OrderDetail } from "@/generated/OrderDetail";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { GET_ORDER } from "../orders/contract";
import styles from "./receiving.module.css";

/** Tracks receipt decisions without equating their completion with readiness for testing. */
export function AccessionProgress({ accession }: { accession: AccessionDetail }) {
  const canOpenOrder = useHasCapability("enter_orders");
  const order = useCommandQuery<OrderDetail>(
    GET_ORDER,
    { input: { orderId: accession.orderId } },
    { enabled: canOpenOrder },
  );
  const pending = accession.specimens.filter(
    (specimen) => specimen.acceptance === "received",
  ).length;
  const held = accession.specimens.filter((specimen) => specimen.acceptance === "held").length;
  const rejected = accession.specimens.filter(
    (specimen) => specimen.acceptance === "rejected",
  ).length;
  return (
    <Section title={pending > 0 ? "Finish receiving" : "Receipt decisions recorded"}>
      <Facts
        layout="compact"
        items={[
          {
            label: "Receipt decisions",
            value: `${accession.specimens.length - pending} of ${accession.specimens.length} recorded`,
          },
          { label: "Held", value: held },
          { label: "Rejected", value: rejected },
        ]}
      />
      {pending > 0 && (
        <p>
          Print the labels you need, then check each container label and record its specimen's
          receipt decision below.
        </p>
      )}
      {held > 0 && (
        <p className={styles.hint}>Held specimens need review under laboratory policy.</p>
      )}
      {canOpenOrder && (
        <QueryContent query={order} what="order summary">
          {(loaded) => (
            <>
              <Link to={`/orders/${loaded.orderId}`}>
                Order {loaded.placerOrderId ?? loaded.orderId}
              </Link>
              <ul className={styles.list} aria-label="Tests in this accession">
                {accession.orderItemIds.map((id) => {
                  const item = loaded.items.find((each) => each.itemId === id);
                  return (
                    <li key={id}>{item ? `${item.orderableCode} ${item.orderableName}` : id}</li>
                  );
                })}
              </ul>
            </>
          )}
        </QueryContent>
      )}
    </Section>
  );
}
