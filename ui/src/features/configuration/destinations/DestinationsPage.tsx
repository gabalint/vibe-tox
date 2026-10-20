import { useMemo } from "react";
import { ErrorState } from "@/components/ErrorState";
import type { ClientView } from "@/generated/ClientView";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { LIST_CLIENTS } from "../contract";
import { Directory } from "../Directory";
import { AddDestinationDialog } from "./AddDestinationDialog";
import { type DestinationView, LIST_DELIVERY_DESTINATIONS } from "./contract";
import { useDestinationActions } from "./DestinationActions";
import { useDestinationFilters } from "./DestinationFilters";
import { DestinationPanel } from "./DestinationPanel";
import { clientText, destinationColumns, indexClients } from "./destinationColumns";

const getRowId = (row: DestinationView) => row.destinationId;
const NO_DESTINATIONS: readonly DestinationView[] = [];
const NO_CLIENTS: readonly ClientView[] = [];

/**
 * Where each client's reports go (REQ-RPT-012), for people with `manage_configuration`. A
 * destination is used only once it is verified, by a second person unless the laboratory's
 * separation-of-duties policy allows its creator to (ADR 0015); a replacement starts
 * unverified again. The engine returns every destination (1–5 per client, at most about
 * 10,000: NFR-33), so the find and the filters run here, and `?open=<id>` always finds the
 * exact destination, as the work queue links it.
 */
export function DestinationsPage() {
  const destinations = useCommandQuery<DestinationView[]>(LIST_DELIVERY_DESTINATIONS, {
    input: {},
  });
  const clients = useCommandQuery<ClientView[]>(LIST_CLIENTS);
  const record = useOpenRecord();
  const clientList = clients.data ?? NO_CLIENTS;
  const rows = destinations.data ?? NO_DESTINATIONS;
  const index = useMemo(() => indexClients(clientList), [clientList]);
  const columns = useMemo(() => destinationColumns(index), [index]);
  const findFields = useMemo(
    () => (row: DestinationView) => [row.label, row.address, clientText(index, row)],
    [index],
  );
  const filters = useDestinationFilters(clientList);
  const { actionsOf, rowActions, dialog } = useDestinationActions(rows);
  return (
    <>
      <Directory
        title="Delivery destinations"
        noun={{ many: "destinations", one: "destination", named: "delivery destinations" }}
        query={destinations}
        columns={columns}
        getRowId={getRowId}
        findFields={findFields}
        findPlaceholder="Label, address or client"
        filters={filters}
        onRowAction={(row, order) => record.open(row.destinationId, order)}
        rowActions={rowActions}
        openRowId={record.openId}
        onOrderChange={record.followOrder}
        notice={
          clients.isError && (
            <ErrorState
              title="The client names could not be loaded"
              error={clients.error}
              onRetry={() => void clients.refetch()}
            />
          )
        }
        panel={openRecordPanel(record, destinations.data, getRowId, (open) => (
          <DestinationPanel
            destination={open}
            destinations={rows}
            clients={index}
            actions={actionsOf(open)}
            record={record}
          />
        ))}
        add={{
          label: "Add destination",
          emptyText: "Add where each client's reports go.",
          dialog: (onClose) => (
            <AddDestinationDialog
              clients={clientList}
              onSaved={(saved) => record.open(saved.destinationId)}
              onClose={onClose}
            />
          ),
        }}
      />
      {dialog}
    </>
  );
}
