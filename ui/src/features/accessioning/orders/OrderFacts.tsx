import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { OrderDetail } from "@/generated/OrderDetail";
import { labelOf } from "@/lib/choices";
import { orNotRecorded } from "../subjects";
import { BILLING_RESPONSIBILITIES, SOURCE_KINDS } from "./labels";
import { clientById, type OrderReferences, practitionerById } from "./references";

interface OrderFactsProps {
  order: OrderDetail;
  references: OrderReferences;
}

/** The requisition as recorded (REQ-ORD-001). */
export function OrderFacts({ order, references }: OrderFactsProps) {
  const { client, location } = clientById(references, order.clientId, order.clientLocationId);
  return (
    <Section title="Requisition">
      <Facts
        layout="compact"
        items={[
          { label: "Received", value: <Timestamp value={order.receivedAt} /> },
          { label: "Client", value: client },
          { label: "Client location", value: orNotRecorded(location) },
          {
            label: "Ordering practitioner",
            value: practitionerById(references, order.practitionerId),
          },
          { label: "Clinic order number", value: placerText(order) },
          {
            label: "Received as",
            value: `${labelOf(SOURCE_KINDS, order.sourceKind)}, document ${orNotRecorded(order.sourceDocumentRef)}`,
          },
          {
            label: "Billing responsibility",
            value: labelOf(BILLING_RESPONSIBILITIES, order.billingResponsibility),
          },
          { label: "Facility setting", value: orNotRecorded(order.facilitySetting) },
          { label: "Template use", value: templateText(order) },
        ]}
      />
    </Section>
  );
}

function placerText({ placerOrderId, placerAuthority }: OrderDetail): string {
  if (placerOrderId === null) return "Not recorded";
  return `${placerOrderId} (issued by ${orNotRecorded(placerAuthority)})`;
}

/** REQ-ORD-006: template use is shown as supplied, never as a per-patient order. */
function templateText({ templateName, templateConfirmedPerPatient }: OrderDetail): string {
  if (templateName === null) return "No template";
  const confirmed = templateConfirmedPerPatient ? "confirmed" : "not confirmed";
  return `Clinic template "${templateName}", ${confirmed} for this patient`;
}
