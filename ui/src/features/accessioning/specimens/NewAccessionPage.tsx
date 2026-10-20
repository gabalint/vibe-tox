import type { ReactNode } from "react";
import { useParams } from "react-router";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Page, PageHeader } from "@/components/Page";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { IntakeSteps } from "../IntakeSteps";
import { GET_ORDER } from "../orders/contract";
import { useOrderAccessionContext } from "../useOrderAccessionContext";
import { AccessionForm } from "./AccessionForm";
import { LIST_SPECIMEN_TYPES } from "./contract";

/** Receive material for an existing order, then verify receipt (REQ-SPC-001/002, WF-02). */
export function NewAccessionPage() {
  const { orderId = "" } = useParams();
  const order = useCommandQuery<OrderDetail>(GET_ORDER, { input: { orderId } });
  const context = useOrderAccessionContext(orderId);
  const specimenTypes = useCommandQuery<SpecimenTypeView[]>(LIST_SPECIMEN_TYPES);
  const failed = [order, context, specimenTypes].find((query) => query.isError);
  let content: ReactNode = failed ? null : <LoadingState label="Preparing specimen receipt…" />;
  // A failed background refresh must not unmount entered values or the pending save's
  // completion callback. Keep the loaded form and show the read failure alongside it.
  if (order.data && context.data && specimenTypes.data) {
    if (specimenTypes.data.length === 0) {
      content = (
        <EmptyState
          title="No specimen types are set up"
          description="An administrator adds specimen types to the catalog first."
        />
      );
    } else {
      content = (
        <AccessionForm
          order={order.data}
          context={context.data}
          specimenTypes={specimenTypes.data}
        />
      );
    }
  }
  return (
    <Page>
      <PageHeader
        title="Receive specimens"
        breadcrumbs={[
          { label: "Order", href: `/orders/${orderId}` },
          { label: "Receive specimens", href: `/orders/${orderId}/accession` },
        ]}
        description="Register the material that arrived. Check labels and record acceptance next."
      />
      <IntakeSteps current="receive" />
      {order.data && <IdentityBanner subjectId={order.data.subjectId} />}
      {failed?.error && (
        <ErrorState
          title={
            order.data && context.data
              ? "Specimen receipt could not be refreshed"
              : "Specimen receipt could not be prepared"
          }
          error={failed.error}
          onRetry={() => void failed.refetch()}
        />
      )}
      {content}
    </Page>
  );
}
