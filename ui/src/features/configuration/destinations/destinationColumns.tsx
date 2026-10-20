import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { ClientLocationView } from "@/generated/ClientLocationView";
import type { ClientView } from "@/generated/ClientView";
import { labelOf } from "@/lib/choices";
import type { DestinationView } from "./contract";
import { CHANNELS, destinationStatus } from "./labels";

/** The clients by ID, so each destination row finds its client without scanning the list. */
export type ClientIndex = ReadonlyMap<string, ClientView>;

export function indexClients(clients: readonly ClientView[]): ClientIndex {
  return new Map(clients.map((client) => [client.id, client]));
}

/** The destination's client and, when it belongs to one, its location, from the client list. */
export function clientOf(
  clients: ClientIndex,
  destination: DestinationView,
): { client: ClientView | undefined; location: ClientLocationView | undefined } {
  const client = clients.get(destination.clientId);
  const location = client?.locations.find((each) => each.id === destination.clientLocationId);
  return { client, location };
}

/** The client's name, and its location's when the destination belongs to one. */
export function clientText(clients: ClientIndex, destination: DestinationView): string {
  const { client, location } = clientOf(clients, destination);
  if (!client) return destination.clientId;
  return location ? `${client.name}, ${location.name}` : client.name;
}

export function Verification({ destination }: { destination: DestinationView }) {
  if (!destination.verified) return "None";
  const { by, at, method, byCreator } = destination.verified;
  return (
    <span>
      {by.displayName}
      {byCreator && " (verified by its creator)"}, <Timestamp value={at} />: {method}
    </span>
  );
}

export function DestinationStatus({ destination }: { destination: DestinationView }) {
  const status = destinationStatus(destination);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}

/** The destination grid's columns; client names come from the client list. */
export function destinationColumns(
  clients: ClientIndex,
): readonly DataGridColumn<DestinationView>[] {
  const client = (row: DestinationView) => clientText(clients, row);
  return [
    {
      id: "label",
      header: "Label",
      identifier: true,
      cell: (row) => row.label,
      sortValue: (row) => row.label,
    },
    { id: "client", header: "Client", cell: client, sortValue: client },
    {
      id: "channel",
      header: "Channel",
      cell: (row) => labelOf(CHANNELS, row.channel),
      sortValue: (row) => row.channel,
      width: 200,
    },
    { id: "address", header: "Address", cell: (row) => row.address ?? "None" },
    {
      id: "state",
      header: "State",
      cell: (row) => <DestinationStatus destination={row} />,
      sortValue: (row) => destinationStatus(row).label,
      width: 150,
    },
    {
      id: "verification",
      header: "Verified by",
      cell: (row) => <Verification destination={row} />,
    },
  ];
}
