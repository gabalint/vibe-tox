import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Tabs } from "@/components/Tabs";
import { Toolbar } from "@/components/Toolbar";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import type { UnroutedMessage } from "@/generated/UnroutedMessage";
import { useCommandQuery } from "@/lib/commands";
import { LIST_CONNECTIONS, LIST_UNROUTED } from "../contract";
import { CONNECTIONS_PATH, connectionPath } from "../paths";
import { ConnectionDialog } from "./ConnectionDialog";
import styles from "./ConnectionsPage.module.css";
import { CONNECTION_COLUMNS } from "./connectionColumns";

/** The tabs of the mockup, each with the message types it lists; All lists every connection. */
const TYPE_TABS: readonly { id: string; label: string; types: readonly PartnerMessageType[] }[] = [
  { id: "all", label: "All", types: [] },
  { id: "orders", label: "Orders in", types: ["hl7_orders"] },
  { id: "results", label: "Results out", types: ["hl7_oru"] },
  { id: "pdfs", label: "Report PDFs", types: ["report_pdf"] },
  { id: "billing", label: "Billing", types: ["billing_json"] },
];

const NO_CONNECTIONS: readonly PartnerConnection[] = [];

/**
 * Connections (ticket #4555; REQ-INT-001, REQ-RPT-003, REQ-RPT-012), as the Interfaces
 * mockup: four counts, a tab per kind of exchange with its count, and every connection with
 * how it is working. Analyzers are not here; they are set up as instruments. The tab stays in
 * `?type=`; a row opens the connection's page.
 */
export function ConnectionsPage() {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const [adding, setAdding] = useState(false);
  const selected = TYPE_TABS.find((tab) => tab.id === params.get("type")) ?? TYPE_TABS[0];
  const connections = useCommandQuery<PartnerConnection[]>(LIST_CONNECTIONS);
  const unrouted = useCommandQuery<UnroutedMessage[]>(LIST_UNROUTED);
  const all = connections.data ?? NO_CONNECTIONS;
  const of = (tab: (typeof TYPE_TABS)[number]) =>
    tab.id === "all" ? all : all.filter((row) => tab.types.includes(row.settings.messageType));
  const rows = selected ? of(selected) : all;
  const loaded = connections.data !== undefined;
  return (
    <Page layout="collection">
      <PageHeader
        title="Connections"
        description="Every exchange with an outside system: orders in, results, report PDFs and billing out. Analyzers are set up in Laboratory setup › Instruments."
      />
      <div className={styles.tiles}>
        <Tile
          label="Active connections"
          value={loaded ? all.filter((row) => row.status === "active").length : null}
        />
        <Tile
          label="With errors"
          tone="danger"
          value={loaded ? all.filter((row) => row.status === "error").length : null}
        />
        {/* A folder never answers, so folder connections never wait for an acknowledgement. */}
        <Tile label="Waiting for acknowledgement" tone="warning" value={loaded ? 0 : null} />
        <Tile label="Unrouted messages" tone="warning" value={unrouted.data?.length ?? null} />
      </div>
      <Tabs
        contentLayout="collection"
        label="Kinds of exchange"
        selectedId={selected?.id ?? "all"}
        tabs={TYPE_TABS.map((tab) => ({
          id: tab.id,
          label: loaded ? `${tab.label} (${of(tab).length})` : tab.label,
          href: tab.id === "all" ? CONNECTIONS_PATH : `${CONNECTIONS_PATH}?type=${tab.id}`,
        }))}
      >
        <Toolbar variant="collection">
          <RefreshControl query={connections} />
          <div className={styles.end}>
            <Button variant="primary" onPress={() => setAdding(true)}>
              Add connection
            </Button>
          </div>
        </Toolbar>
        <QueryContent
          query={connections}
          what="connections"
          loadingRows={6}
          empty={{
            when: () => rows.length === 0,
            title: "No connections",
            description:
              selected?.id !== "orders"
                ? "Add a connection to send HL7 results, report PDFs or billing handoff files to a folder or SFTP server, or to receive HL7 orders."
                : "Add a connection to read HL7 orders from a clinic's folder or SFTP server.",
          }}
        >
          {() => (
            <DataGrid
              label="Connections"
              columns={CONNECTION_COLUMNS}
              rows={rows}
              getRowId={(row) => row.id}
              onRowAction={(row) => navigate(connectionPath(row.id))}
              fill
            />
          )}
        </QueryContent>
      </Tabs>
      {adding && (
        <ConnectionDialog
          connection={null}
          onClose={() => setAdding(false)}
          onSaved={(saved) => navigate(connectionPath(saved.connection.id))}
        />
      )}
    </Page>
  );
}

/** One count; a non-zero count of something needing attention takes its tone. */
function Tile({
  label,
  value,
  tone,
}: {
  label: string;
  value: number | null;
  tone?: "danger" | "warning";
}) {
  return (
    <div className={styles.tile}>
      <span className={styles.tileLabel}>{label}</span>
      <span className={styles.tileValue} data-tone={value ? tone : undefined}>
        {value ?? "…"}
      </span>
    </div>
  );
}
