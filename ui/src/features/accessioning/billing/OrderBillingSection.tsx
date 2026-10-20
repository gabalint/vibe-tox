import { useState } from "react";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { OrderBillingView } from "@/generated/OrderBillingView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { GET_ORDER_BILLING } from "./contract";
import { EvidenceList } from "./EvidenceList";
import { READINESS } from "./labels";
import { OrderBillingFacts } from "./OrderBillingFacts";
import { OrderContextDialog } from "./OrderContextDialog";
import { OrderEvidenceDialog } from "./OrderEvidenceDialog";
import { ReviewItems } from "./ReviewItems";

export function OrderBillingSection({
  orderId,
  subjectId,
}: {
  orderId: string;
  subjectId: string;
}) {
  const [opened, setOpened] = useState(false);
  return (
    <Section title="Order billing">
      {opened ? (
        <OrderBillingQuery orderId={orderId} subjectId={subjectId} />
      ) : (
        <>
          <p className={styles.hint}>
            Review this order's selected billing facts, evidence and financial follow-up.
          </p>
          <Button onPress={() => setOpened(true)}>Open order billing</Button>
        </>
      )}
    </Section>
  );
}

function OrderBillingQuery({ orderId, subjectId }: { orderId: string; subjectId: string }) {
  const query = useCommandQuery<OrderBillingView>(GET_ORDER_BILLING, { input: { orderId } });
  return (
    <>
      <div>
        <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
          Refresh order billing
        </Button>
      </div>
      <QueryContent query={query} what="order billing">
        {(view) => <OrderBillingContent view={view} subjectId={subjectId} />}
      </QueryContent>
    </>
  );
}

export function OrderBillingContent({
  view,
  subjectId,
}: {
  view: OrderBillingView;
  subjectId: string;
}) {
  const [dialog, setDialog] = useState<"context" | "evidence" | null>(null);
  const context = view.context;
  const canEdit = view.allowedActions.includes("set_order_billing_context");
  const close = () => setDialog(null);
  return (
    <div className={styles.stack}>
      {view.readinessIssues.length > 0 && (
        <div>
          <h3 className={styles.subheading}>Financial information to review</h3>
          <ul>
            {view.readinessIssues.map((issue) => (
              <li key={issue}>{labelOf(READINESS, issue)}</li>
            ))}
          </ul>
        </div>
      )}
      <OrderBillingFacts context={context} />
      {canEdit && (
        <Button onPress={() => setDialog("context")}>
          {context ? "Correct order billing selection" : "Confirm order billing selection"}
        </Button>
      )}
      <Section
        title="Order financial evidence"
        variant="plain"
        actions={
          canEdit &&
          context && <Button onPress={() => setDialog("evidence")}>Add order evidence</Button>
        }
      >
        <EvidenceList evidence={context?.financialEvidence ?? []} />
      </Section>
      <ReviewItems view={view} />
      {dialog === "context" && (
        <OrderContextDialog view={view} subjectId={subjectId} onClose={close} />
      )}
      {dialog === "evidence" && context && (
        <OrderEvidenceDialog context={context} revision={view.orderRevision} onClose={close} />
      )}
    </div>
  );
}
