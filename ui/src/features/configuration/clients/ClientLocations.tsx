import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { AddClientLocation } from "@/generated/AddClientLocation";
import type { ClientLocationView } from "@/generated/ClientLocationView";
import type { ClientView } from "@/generated/ClientView";
import type { CreateClient } from "@/generated/CreateClient";
import { CommandDialog } from "@/lib/CommandDialog";
import { useHasCapability } from "@/lib/commands";
import { CodedNameFields, trimCodedName } from "../CodedNameFields";
import { LIST_CLIENTS } from "../contract";
import { useDialog } from "../useDialog";
import { CorrectClientDialog } from "./CorrectClientDialog";
import { DirectoryHistory } from "./DirectoryHistory";

const COLUMNS: readonly DataGridColumn<ClientLocationView>[] = [
  { id: "code", header: "Code", cell: (row) => row.code, sortValue: (row) => row.code, width: 180 },
  {
    id: "state",
    header: "Availability",
    cell: (row) => (row.active === false ? "Retired" : "Active"),
  },
  { id: "name", header: "Name", cell: (row) => row.name, sortValue: (row) => row.name },
];

const getRowId = (location: ClientLocationView) => location.id;

/**
 * Locations retain identity and correction history when their facts or availability change.
 */
export function ClientLocations({ client }: { client: ClientView }) {
  const dialog = useDialog();
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const canEdit = useHasCapability("manage_configuration");
  const location = client.locations.find((row) => row.id === selected);
  return (
    <Section title="Locations" actions={<Button onPress={dialog.open}>Add location</Button>}>
      {client.locations.length === 0 ? (
        <p>No locations yet.</p>
      ) : (
        <DataGrid
          label={`Locations of ${client.name}`}
          columns={COLUMNS}
          rows={client.locations}
          getRowId={getRowId}
          onRowAction={(row) => setSelected(row.id)}
        />
      )}
      {location && (
        <DetailPanel
          identifier={location.name}
          onClose={() => setSelected(null)}
          actions={canEdit && <Button onPress={() => setEditing(true)}>Correct location</Button>}
        >
          <Facts
            items={[
              { label: "Code", value: location.code },
              { label: "Availability", value: location.active === false ? "Retired" : "Active" },
              { label: "Revision", value: location.revision },
            ]}
          />
          <DirectoryHistory key={location.id} record={{ kind: "location", id: location.id }} />
        </DetailPanel>
      )}
      {editing && location && (
        <CorrectClientDialog kind="location" record={location} onClose={() => setEditing(false)} />
      )}
      {dialog.isOpen && (
        <CommandDialog<CreateClient, AddClientLocation>
          title={`Add location to ${client.name}`}
          submitLabel="Add location"
          command={{ name: "add_client_location", invalidates: [LIST_CLIENTS] }}
          defaultValues={{ code: "", name: "" }}
          toInput={(values) => ({ clientId: client.id, ...trimCodedName(values) })}
          successTitle={(values) => `Location ${values.name.trim()} added`}
          errorTitle="Not saved"
          onClose={dialog.close}
        >
          <CodedNameFields />
        </CommandDialog>
      )}
    </Section>
  );
}
