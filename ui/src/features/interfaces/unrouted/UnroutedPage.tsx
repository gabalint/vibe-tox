import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Toolbar } from "@/components/Toolbar";
import type { UnroutedMessage } from "@/generated/UnroutedMessage";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { useOpenRecord } from "@/lib/openRecord";
import { LIST_UNROUTED } from "../contract";
import { MESSAGE_TYPES, recordText, valuesText } from "../labels";
import { UnroutedDetails } from "./UnroutedDetails";

const HOUR_MS = 60 * 60 * 1000;

/** How long a message has waited, such as "3 h" or "2 d". */
function waited(since: string, now: number): string {
  const hours = Math.floor((now - Date.parse(since)) / HOUR_MS);
  if (hours < 1) return "under 1 h";
  return hours < 48 ? `${hours} h` : `${Math.floor(hours / 24)} d`;
}

const messageId = (row: UnroutedMessage) => row.messageId;

/**
 * Unrouted messages (ticket #4555; REQ-RPT-003), as the Interfaces mockup: outgoing messages
 * that matched no connection's rules. Nothing is lost: each waits here, oldest first, until
 * someone sends it once, routes it again after changing a rule, records another way it was
 * delivered, or dismisses it with a reason. The chosen row (`?open=`, the oldest by default)
 * shows its details and decisions below the table; every decision is audited and shows in
 * Logs.
 */
export function UnroutedPage() {
  const record = useOpenRecord();
  const formatTime = useFormatTime();
  const messages = useCommandQuery<UnroutedMessage[]>(LIST_UNROUTED);
  const rows = messages.data ?? [];
  const chosen = rows.find((row) => row.messageId === record.openId) ?? rows[0];
  const now = Date.now();
  const columns: readonly DataGridColumn<UnroutedMessage>[] = [
    {
      id: "since",
      header: "Waiting since",
      cell: (row) => `${formatTime(row.createdAt)} (${waited(row.createdAt, now)})`,
      sortValue: (row) => row.createdAt,
      width: 230,
    },
    { id: "message", header: "Message", cell: (row) => MESSAGE_TYPES[row.messageType], width: 130 },
    { id: "record", header: "Record", cell: recordText, identifier: true },
    { id: "client", header: "Client", cell: (row) => valuesText(row.facts.clients), wrap: true },
    {
      id: "practitioner",
      header: "Practitioner",
      cell: (row) => valuesText(row.facts.practitioners),
      wrap: true,
    },
    { id: "reason", header: "Reason", cell: () => "No rule matched", width: 150 },
    { id: "next", header: "Next action", cell: () => "Send to a connection", width: 180 },
  ];
  return (
    <Page layout="collection">
      <PageHeader
        title="Unrouted messages"
        description="Outbound messages that matched no connection rule. Nothing is lost: each waits here until someone decides."
      />
      <Toolbar variant="collection">
        <RefreshControl query={messages} />
      </Toolbar>
      <QueryContent
        query={messages}
        what="unrouted messages"
        loadingRows={4}
        empty={{
          when: () => rows.length === 0,
          title: "No unrouted messages",
          description: "Every message matched a connection, or was decided.",
        }}
      >
        {() => (
          <>
            <DataGrid
              label="Unrouted messages"
              columns={columns}
              rows={rows}
              getRowId={messageId}
              openRowId={chosen?.messageId ?? null}
              onOrderChange={record.followOrder}
              onRowAction={(row, order) => record.open(row.messageId, order)}
            />
            {chosen && <UnroutedDetails key={chosen.messageId} message={chosen} />}
          </>
        )}
      </QueryContent>
    </Page>
  );
}
