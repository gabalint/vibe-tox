import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { ClientView } from "@/generated/ClientView";
import type { OrderBillingContext } from "@/generated/OrderBillingContext";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { BILLING_RESPONSIBILITIES } from "../orders/labels";
import { ContactFacts, PartyFacts } from "./BillingFacts";

export function OrderBillingFacts({ context }: { context: OrderBillingContext | null }) {
  if (!context) return <p>No order-specific billing selection confirmed.</p>;
  const selection = context.selection;
  return (
    <>
      <Facts
        items={[
          { label: "Billing revision", value: context.version },
          {
            label: "Bill-to client account",
            value: selection.clientId ? (
              <ClientName clientId={selection.clientId} />
            ) : (
              "None selected"
            ),
          },
          {
            label: "Responsibility",
            value: labelOf(BILLING_RESPONSIBILITIES, selection.responsibility),
          },
          {
            label: "Patient profile revision",
            value: selection.profileVersion ?? "None selected",
          },
          {
            label: "Coverage priority",
            value: selection.coverages.length ? (
              <ol>
                {selection.coverages.map((coverage) => (
                  <li key={coverage.key}>
                    {coverage.payerName} · {coverage.memberId ?? "No member identifier"}
                  </li>
                ))}
              </ol>
            ) : (
              "None selected"
            ),
          },
          { label: "Guarantor", value: selection.guarantor?.name ?? "None selected" },
          { label: "Source", value: context.source },
          { label: "Reason", value: context.reason },
          { label: "Confirmed", value: <Timestamp value={context.recordedAt} /> },
        ]}
      />
      <details>
        <summary>Selected financial details</summary>
        {selection.contact && <ContactFacts contact={selection.contact} />}
        {selection.guarantor && (
          <details>
            <summary>Selected guarantor</summary>
            <PartyFacts party={selection.guarantor} />
          </details>
        )}
        {selection.coverages.map((coverage, index) => (
          <details key={coverage.key}>
            <summary>
              Coverage {index + 1}: {coverage.payerName}
            </summary>
            <Facts
              items={[
                { label: "Member identifier", value: coverage.memberId ?? "Not supplied" },
                { label: "Payer identifier", value: coverage.payerId ?? "Not supplied" },
                { label: "Plan", value: coverage.planName ?? "Not supplied" },
                { label: "Group", value: coverage.groupNumber ?? "Not supplied" },
                {
                  label: "Coverage dates",
                  value: `${coverage.effectiveFrom ?? "Not supplied"} to ${coverage.effectiveUntil ?? "Not supplied"}`,
                },
                { label: "Source", value: coverage.source },
              ]}
            />
            {coverage.subscriber && <PartyFacts party={coverage.subscriber} />}
          </details>
        ))}
      </details>
    </>
  );
}

function ClientName({ clientId }: { clientId: string }) {
  const query = useCommandQuery<ClientView[]>("list_clients");
  return (
    <QueryContent query={query} what="billing client accounts">
      {(clients) =>
        clients.find((client) => client.id === clientId)?.name ?? "Selected account unavailable"
      }
    </QueryContent>
  );
}
