import { useNavigate } from "react-router";
import type { DataGridColumn } from "@/components/DataGrid";
import type { ClientView } from "@/generated/ClientView";
import type { CreateClient } from "@/generated/CreateClient";
import { CommandDialog } from "@/lib/CommandDialog";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { CodedNameFields, trimCodedName } from "../CodedNameFields";
import { codedColumns } from "../catalogColumns";
import { LIST_CLIENTS } from "../contract";
import { byId, Directory } from "../Directory";
import { clientPath } from "./clientValues";

const COLUMNS: readonly DataGridColumn<ClientView>[] = [
  ...codedColumns<ClientView>(),
  {
    id: "state",
    header: "Availability",
    cell: (row) => (row.active === false ? "Retired" : "Active"),
  },
  {
    id: "locations",
    header: "Locations",
    cell: (row) => row.locations.length,
    sortValue: (row) => row.locations.length,
    align: "end",
    width: 120,
  },
];

const FIND_FIELDS = (client: ClientView) => [client.code, client.name];

/**
 * The ordering clinics (REQ-ORD-001 references), for people with `manage_configuration`.
 * A row opens the client's page, with its locations and delivery destinations. The engine
 * returns every client (20–200, at most about 2,000), so the find runs here.
 */
export function ClientsPage() {
  const query = useCommandQuery<ClientView[]>(LIST_CLIENTS);
  const navigate = useNavigate();
  const open = useOpenRecordPage(clientPath, "Clients");
  return (
    <Directory
      title="Clients"
      noun={{ many: "clients", one: "client" }}
      query={query}
      columns={COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code or name"
      onRowAction={(row, order) => open(row.id, order)}
      add={{
        label: "Add client",
        emptyText: "Add the clinics that send orders.",
        dialog: (onClose) => (
          <CommandDialog<CreateClient, CreateClient, string>
            title="Add client"
            submitLabel="Add client"
            command={{ name: "create_client", invalidates: [LIST_CLIENTS] }}
            defaultValues={{ code: "", name: "" }}
            toInput={trimCodedName}
            successTitle={(values) => `Client ${values.name.trim()} added`}
            onDone={(clientId) => void navigate(clientPath(clientId))}
            errorTitle="Not saved"
            onClose={onClose}
          >
            <CodedNameFields />
          </CommandDialog>
        ),
      }}
    />
  );
}
