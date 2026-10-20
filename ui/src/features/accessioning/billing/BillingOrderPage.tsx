import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { OrderBillingView } from "@/generated/OrderBillingView";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { GET_ORDER_BILLING } from "./contract";
import { OrderBillingContent } from "./OrderBillingSection";
import { PatientBillingSection } from "./PatientBillingSection";

/** Financial access uses a scoped read; no clinical order or subject query is required. */
export function BillingOrderPage() {
  const { orderId = "" } = useParams();
  const query = useCommandQuery<OrderBillingView>(GET_ORDER_BILLING, { input: { orderId } });
  return (
    <Page>
      <PageHeader
        title="Order billing"
        actions={
          <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
            Refresh order billing
          </Button>
        }
        breadcrumbs={[
          { label: "Billing follow-up", href: "/billing/review" },
          { label: "Order billing", href: `/billing/orders/${orderId}` },
        ]}
        description="Financial selection, evidence and follow-up for this order. Viewing is recorded in the audit trail."
      />
      <QueryContent query={query} what="order billing">
        {(view) => (
          <>
            <IdentityBanner subject={view.subject} />
            <Section title="Order billing selection">
              <OrderBillingContent key={orderId} view={view} subjectId={view.subject.subjectId} />
            </Section>
            <PatientBillingSection
              key={view.subject.subjectId}
              subjectId={view.subject.subjectId}
            />
          </>
        )}
      </QueryContent>
    </Page>
  );
}
