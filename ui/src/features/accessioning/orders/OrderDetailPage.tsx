import { useParams, useSearchParams } from "react-router";
import { Alert } from "@/components/Alert";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { StatusBadge } from "@/components/StatusBadge";
import type { OrderDetail } from "@/generated/OrderDetail";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import { OrderBillingSection } from "../billing/OrderBillingSection";
import { IntakeSteps } from "../IntakeSteps";
import { useOrderAccessionContext } from "../useOrderAccessionContext";
import { ClarificationHolds } from "./ClarificationHolds";
import { GET_ORDER } from "./contract";
import { orderStatus } from "./labels";
import { OrderAccessions } from "./OrderAccessions";
import { OrderActions } from "./OrderActions";
import { OrderEvidence } from "./OrderEvidence";
import { OrderFacts } from "./OrderFacts";
import { OrderItems } from "./OrderItems";
import { OrderMaintenance } from "./OrderMaintenance";
import { OrderReferencesGate, useOrderReferences } from "./OrderReferencesGate";
import { orderPath } from "./orderColumns";

const ORDERS = { label: "Orders", href: "/orders" };

/**
 * One order: its requisition, tests, evidence and clarification holds, with the decisions
 * the engine allows now (REQ-ORD-001, REQ-ORD-005). Viewing it is an audited access. The
 * header stays in place while another order loads, so focus stays on Previous or Next.
 */
export function OrderDetailPage() {
  const { orderId = "" } = useParams();
  const [params] = useSearchParams();
  const receiveNext = params.get("next") === "receive";
  const order = useCommandQuery<OrderDetail>(GET_ORDER, { input: { orderId } });
  const accessionContext = useOrderAccessionContext(orderId);
  const { list, sequence } = useRecordSequence(orderId, ORDERS, orderPath);
  const { references } = useOrderReferences();
  const data = order.data;
  const title = data ? orderTitle(data) : "Order";
  const status = data && orderStatus(data.status);
  return (
    <Page>
      <PageHeader
        title={title}
        breadcrumbs={[list, { label: title, href: orderPath(orderId) }]}
        sequence={sequence}
        status={status && <StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
        description={
          receiveNext
            ? "Review the saved request before receiving specimens."
            : `${data ? `Evidence version ${data.version}. ` : ""}${AUDITED}`
        }
        actions={
          data && (
            <OrderActions
              order={data}
              references={references}
              accessionContext={accessionContext}
              receiveNext={receiveNext}
            />
          )
        }
      />
      <RefreshControl query={order} />
      {receiveNext && <IntakeSteps current="order" />}
      {data && <IdentityBanner subjectId={data.subjectId} />}
      {data && receiveNext && (
        <Alert tone="info" title="Order saved">
          {data.status === "received"
            ? "Review the request and any holds, then explicitly accept the order to continue receiving specimens."
            : "This saved order is ready for review. The specimen status below shows whether receiving can continue."}
        </Alert>
      )}
      <QueryContent query={order} what="order">
        {(loaded) => <OrderRecord order={loaded} accessionContext={accessionContext} />}
      </QueryContent>
    </Page>
  );
}

const AUDITED = "Viewing this order is recorded in the audit trail.";

/** The order has no laboratory number: it is named by the source's order number, if any. */
function orderTitle(order: OrderDetail): string {
  return order.placerOrderId === null ? "Order" : `Order ${order.placerOrderId}`;
}

function OrderRecord({
  order,
  accessionContext,
}: {
  order: OrderDetail;
  accessionContext: ReturnType<typeof useOrderAccessionContext>;
}) {
  return (
    <>
      <ClarificationHolds order={order} />
      <OrderAccessions order={order} context={accessionContext} />
      <OrderReferencesGate>
        {(loaded) => (
          <>
            <OrderFacts order={order} references={loaded} />
            <OrderItems order={order} references={loaded} />
          </>
        )}
      </OrderReferencesGate>
      <OrderEvidence evidence={order.evidence} />
      <OrderMaintenance order={order} />
      <OrderBillingSection
        key={order.orderId}
        orderId={order.orderId}
        subjectId={order.subjectId}
      />
    </>
  );
}
