import { Link, useSearchParams } from "react-router";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { labelOf } from "@/lib/choices";
import type { OpenRecord } from "@/lib/openRecord";
import type { DestinationView } from "./contract";
import { type ClientIndex, clientOf, DestinationStatus, Verification } from "./destinationColumns";
import { CHANNELS } from "./labels";

interface DestinationPanelProps {
  destination: DestinationView;
  /** Every destination, to name the one this replaced. */
  destinations: readonly DestinationView[];
  clients: ClientIndex;
  actions: readonly ActionItem[];
  record: OpenRecord;
}

/**
 * One destination in view mode: where reports go, who verified it and how, and the actions
 * the engine allows. Its client links to the client's page.
 */
export function DestinationPanel({
  destination,
  destinations,
  clients,
  actions,
  record,
}: DestinationPanelProps) {
  const { client, location } = clientOf(clients, destination);
  const replaced = destinations.find((each) => each.destinationId === destination.replacesId);
  const [params] = useSearchParams();
  const opening = (id: string) => {
    const next = new URLSearchParams(params);
    next.set("open", id);
    return `?${next}`;
  };
  return (
    <DetailPanel
      identifier={destination.label}
      status={<DestinationStatus destination={destination} />}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    >
      <Facts
        items={[
          {
            label: "Client",
            value: (
              <Link to={`/configuration/clients/${destination.clientId}`}>
                {client?.name ?? destination.clientId}
              </Link>
            ),
          },
          { label: "Location", value: location?.name ?? "Whole client" },
          { label: "Channel", value: labelOf(CHANNELS, destination.channel) },
          { label: "Address", value: destination.address ?? "None" },
          { label: "Verified", value: <Verification destination={destination} /> },
          { label: "Added by", value: destination.createdBy.displayName },
          ...(replaced
            ? [
                {
                  label: "Replaces",
                  value: (
                    <Link to={opening(replaced.destinationId)} replace>
                      {replaced.label}
                    </Link>
                  ),
                },
              ]
            : []),
          ...(destination.retireReason
            ? [{ label: "Retired because", value: destination.retireReason }]
            : []),
        ]}
      />
    </DetailPanel>
  );
}
