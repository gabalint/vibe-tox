import { useMemo } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { ClientView } from "@/generated/ClientView";
import { useCommandQuery } from "@/lib/commands";
import { AddDestinationDialog } from "../destinations/AddDestinationDialog";
import { type DestinationView, LIST_DELIVERY_DESTINATIONS } from "../destinations/contract";
import { destinationColumns, indexClients } from "../destinations/destinationColumns";
import { useDialog } from "../useDialog";

const getRowId = (row: DestinationView) => row.destinationId;

/**
 * Where this client's reports go (REQ-RPT-012): its destinations that are not retired, 1–5
 * per client. A row opens the destination in the delivery destinations directory, where
 * its actions are.
 */
export function ClientDestinations({
  client,
  clients,
}: {
  client: ClientView;
  clients: readonly ClientView[];
}) {
  const query = useCommandQuery<DestinationView[]>(LIST_DELIVERY_DESTINATIONS, {
    input: { clientId: client.id },
  });
  const navigate = useNavigate();
  const dialog = useDialog();
  const columns = useMemo(() => destinationColumns(indexClients(clients)), [clients]);
  return (
    <Section
      title="Delivery destinations"
      actions={<Button onPress={dialog.open}>Add destination</Button>}
    >
      <QueryContent
        query={query}
        what="delivery destinations"
        loadingRows={3}
        empty={{
          when: (rows) => rows.every((row) => row.retired),
          title: "No delivery destinations yet",
          description: "Add where this client's reports go.",
        }}
      >
        {(rows) => (
          <ActiveDestinations
            label={`Delivery destinations of ${client.name}`}
            columns={columns}
            rows={rows}
            onOpen={(row) => void navigate(`/configuration/destinations?open=${row.destinationId}`)}
          />
        )}
      </QueryContent>
      {dialog.isOpen && (
        <AddDestinationDialog clients={clients} presetClientId={client.id} onClose={dialog.close} />
      )}
    </Section>
  );
}

interface ActiveDestinationsProps {
  label: string;
  columns: readonly DataGridColumn<DestinationView>[];
  rows: readonly DestinationView[];
  onOpen: (row: DestinationView) => void;
}

/** The grid of the destinations that are not retired; the filtered rows are kept between renders. */
function ActiveDestinations({ label, columns, rows, onOpen }: ActiveDestinationsProps) {
  const active = useMemo(() => rows.filter((row) => !row.retired), [rows]);
  return (
    <DataGrid
      label={label}
      columns={columns}
      rows={active}
      getRowId={getRowId}
      onRowAction={onOpen}
    />
  );
}
