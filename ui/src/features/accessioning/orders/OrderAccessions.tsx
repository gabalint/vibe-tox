import { Link } from "react-router";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { OrderDetail } from "@/generated/OrderDetail";
import styles from "../accessioning.module.css";
import type { useOrderAccessionContext } from "../useOrderAccessionContext";
import { orderedTestName } from "./references";

interface OrderAccessionsProps {
  order: OrderDetail;
  context: ReturnType<typeof useOrderAccessionContext>;
}

/** Material already linked to the order, and the engine's reasons for remaining work. */
export function OrderAccessions({ order, context }: OrderAccessionsProps) {
  return (
    <Section title="Specimens and accessions">
      <QueryContent query={context} what="specimen receiving status">
        {(data) => (
          <>
            {data.accessions.length === 0 ? (
              <p>No specimens have been accessioned for this order.</p>
            ) : (
              <ul className={styles.list}>
                {data.accessions.map((accession) => (
                  <li key={accession.accessionId}>
                    <Link to={`/accessions/${accession.accessionId}`}>
                      Accession {accession.accessionNumber}
                    </Link>
                    {" · "}
                    {accession.specimenCount}{" "}
                    {accession.specimenCount === 1 ? "specimen" : "specimens"}
                    {" · "}
                    {accession.pendingReceiptCount > 0
                      ? `Receipt review needed: ${accession.pendingReceiptCount}`
                      : "Receipt decisions recorded"}
                    {" · "}
                    <Timestamp value={accession.createdAt} />
                  </li>
                ))}
              </ul>
            )}
            <ul className={styles.list} aria-label="Specimen receiving status by test">
              {data.items.map((item) => {
                const ordered = order.items.find((each) => each.itemId === item.orderItemId);
                return (
                  <li key={item.orderItemId}>
                    {ordered ? orderedTestName(ordered) : "Ordered test"}
                    {" — "}
                    {item.eligible
                      ? `Awaiting material (${item.requiredSpecimenTypeCodes.join(", ")})`
                      : item.reason}
                    {item.accessionId && (
                      <>
                        {" · "}
                        <Link to={`/accessions/${item.accessionId}`}>
                          Accession {item.accessionNumber}
                        </Link>
                      </>
                    )}
                  </li>
                );
              })}
            </ul>
          </>
        )}
      </QueryContent>
    </Section>
  );
}
