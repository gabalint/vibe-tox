import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import type { ClientView } from "@/generated/ClientView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useRecordSequence } from "@/lib/recordSequence";
import { LIST_CLIENTS } from "../contract";
import { ClientDestinations } from "./ClientDestinations";
import { ClientLocations } from "./ClientLocations";
import { CorrectClientDialog } from "./CorrectClientDialog";
import { clientPath } from "./clientValues";
import { DirectoryHistory } from "./DirectoryHistory";

const HOME = { label: "Clients", href: "/configuration/clients" };

/**
 * One client (REQ-ORD-001 references): its details, its locations, and where its reports go
 * (REQ-RPT-012). The client comes from the client list, which the directory has usually
 * loaded already; the engine has no single-client read.
 */
export function ClientPage() {
  const { clientId = "" } = useParams();
  const clients = useCommandQuery<ClientView[]>(LIST_CLIENTS);
  const { list, sequence } = useRecordSequence(clientId, HOME, clientPath);
  const client = clients.data?.find((each) => each.id === clientId);
  return (
    <Page>
      <PageHeader
        title={client ? `${client.code} ${client.name}` : "Client"}
        breadcrumbs={[list, { label: client?.code ?? "Client", href: clientPath(clientId) }]}
        sequence={sequence}
        actions={<RefreshControl query={clients} />}
      />
      <QueryContent query={clients} what="client">
        {(all) => {
          const found = all.find((each) => each.id === clientId);
          return found ? (
            <ClientSections client={found} clients={all} />
          ) : (
            <EmptyState
              title="Client not found"
              description="No client has this identifier. Open the client from the list of clients."
            />
          );
        }}
      </QueryContent>
    </Page>
  );
}

function ClientSections({
  client,
  clients,
}: {
  client: ClientView;
  clients: readonly ClientView[];
}) {
  const [editing, setEditing] = useState(false);
  const canEdit = useHasCapability("manage_configuration");
  return (
    <>
      <Section
        title="Details"
        actions={canEdit && <Button onPress={() => setEditing(true)}>Correct client</Button>}
      >
        <Facts
          layout="compact"
          items={[
            { label: "Code", value: client.code },
            { label: "Name", value: client.name },
            { label: "Availability", value: client.active === false ? "Retired" : "Active" },
            { label: "Revision", value: client.revision },
          ]}
        />
        <DirectoryHistory key={client.id} record={{ kind: "client", id: client.id }} />
        {editing && (
          <CorrectClientDialog record={client} kind="client" onClose={() => setEditing(false)} />
        )}
      </Section>
      <ClientLocations client={client} />
      <ClientDestinations client={client} clients={clients} />
    </>
  );
}
