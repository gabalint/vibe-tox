import { useParams } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Tabs } from "@/components/Tabs";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import { useCommandQuery } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { GET_CONNECTION } from "../contract";
import { connectionDescription } from "../labels";
import { ConnectionMessages } from "../logs/MessagesSection";
import { CONNECTIONS_PATH, connectionPath } from "../paths";
import { ConnectionActions } from "./ConnectionActions";
import { ConnectionHistory } from "./ConnectionHistory";
import { CodeMappings, ConnectionOverview, ConnectionRules } from "./ConnectionOverview";
import { CheckBadge, StatusOf } from "./connectionColumns";

const TABS = [
  { id: "overview", label: "Overview" },
  { id: "rules", label: "Rules" },
  { id: "mappings", label: "Code mappings" },
  { id: "logs", label: "Logs" },
  { id: "history", label: "History" },
] as const;

/**
 * One connection (ticket #4555; REQ-INT-001, REQ-RPT-012), as the Interfaces mockup: its
 * status and destination check beside the name, a one-line description, its actions in one
 * row, and tabs for its settings and checks, rules, code mappings, messages and history.
 */
export function ConnectionPage() {
  const { connectionId = "" } = useParams();
  const detail = useCommandQuery<PartnerConnectionDetail>(GET_CONNECTION, {
    input: { connectionId },
  });
  const { selectedId: tab, tabs } = useRouteTabs(connectionPath(connectionId), TABS);
  const connection = detail.data?.connection;
  const title = connection?.settings.name ?? "Connection";
  return (
    <Page>
      <PageHeader
        title={title}
        breadcrumbs={[
          { label: "Connections", href: CONNECTIONS_PATH },
          { label: title, href: connectionPath(connectionId) },
        ]}
        status={
          connection && (
            <>
              <StatusOf connection={connection} /> <CheckBadge connection={connection} />
            </>
          )
        }
        description={
          connection &&
          connectionDescription(connection.settings.messageType, connection.settings.partner)
        }
      />
      <QueryContent query={detail} what="connection">
        {(loaded) => (
          <>
            <ConnectionActions connection={loaded.connection} />
            <Tabs label="Connection" tabs={tabs} selectedId={tab}>
              {tab === "overview" && (
                <ConnectionOverview
                  connection={loaded.connection}
                  lastReason={loaded.history[0]?.reason ?? null}
                />
              )}
              {tab === "rules" && <ConnectionRules connection={loaded.connection} />}
              {tab === "mappings" && <CodeMappings connection={loaded.connection} />}
              {tab === "logs" && <ConnectionMessages connectionId={loaded.connection.id} />}
              {tab === "history" && <ConnectionHistory history={loaded.history} />}
            </Tabs>
          </>
        )}
      </QueryContent>
    </Page>
  );
}
