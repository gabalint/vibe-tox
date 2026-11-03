import type { UseQueryResult } from "@tanstack/react-query";
import { useNavigate } from "react-router";
import { DataGrid } from "@/components/DataGrid";
import { ErrorState } from "@/components/ErrorState";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { useToast } from "@/components/Toast";
import { Toolbar } from "@/components/Toolbar";
import type { CommandError } from "@/generated/CommandError";
import type { PartnerMessagePage } from "@/generated/PartnerMessagePage";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { ResendPartnerDelivery } from "@/generated/ResendPartnerDelivery";
import { allowedRowActions } from "@/lib/allowedActions";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import { LIST_MESSAGES, MESSAGE_QUERIES, RESEND_DELIVERY } from "../contract";
import { recordText } from "../labels";
import { connectionPath, UNROUTED_PATH } from "../paths";
import { messageColumns } from "./messageColumns";

const ACTIONS = [{ id: RESEND_DELIVERY, label: "Send again" }];

interface MessagesGridProps {
  /** The log query, for its loading, error and limit states. */
  query: UseQueryResult<PartnerMessagePage, CommandError>;
  /** The rows to show: the query's rows, narrowed by the screen's filters. */
  rows: readonly PartnerMessageRow[];
}

/**
 * Messages to outside systems, newest first, at most 500 (REQ-INT-002, REQ-RPT-011): what the
 * transport can prove and what the partner's system confirmed, never a bare delivery claim. A
 * row opens its connection, or Unrouted messages for one that matched none. A message whose
 * attempts all failed can be sent again; that never makes a new message or report.
 */
export function MessagesGrid({ query, rows }: MessagesGridProps) {
  const navigate = useNavigate();
  const toast = useToast();
  const formatTime = useFormatTime();
  const resend = useCommandMutation<unknown, { input: ResendPartnerDelivery }>(RESEND_DELIVERY, {
    invalidates: MESSAGE_QUERIES,
  });
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: PartnerMessageRow) => recordText(row),
    allowed: (row) => row.allowedActions,
    onAction: (_, row) => {
      if (row.deliveryId === null) return;
      resend.mutate(
        { input: { deliveryId: row.deliveryId } },
        { onSuccess: () => toast.show({ tone: "success", title: "Waiting to be written again" }) },
      );
    },
  });
  const open = (row: PartnerMessageRow) =>
    navigate(
      row.connection
        ? connectionPath(row.connection.id)
        : `${UNROUTED_PATH}?open=${encodeURIComponent(row.messageId)}`,
    );
  return (
    <>
      {query.data?.more && (
        <p className={styles.hint}>
          The 500 newest messages are shown. Narrow the period to see older ones.
        </p>
      )}
      {resend.error && <ErrorState title="Not sent again" error={resend.error} />}
      <QueryContent
        query={query}
        what="messages"
        loadingRows={8}
        empty={{
          when: () => rows.length === 0,
          title: "No messages",
          description: "Messages appear here when a billing handoff is created for a connection.",
        }}
      >
        {() => (
          <DataGrid
            label="Messages"
            columns={messageColumns(formatTime)}
            rows={rows}
            getRowId={(row) => row.deliveryId ?? row.messageId}
            onRowAction={open}
            rowActions={rowActions}
            fill
          />
        )}
      </QueryContent>
    </>
  );
}

/** One connection's messages, with Refresh and the count, for its Logs tab. */
export function ConnectionMessages({ connectionId }: { connectionId: string }) {
  const messages = useCommandQuery<PartnerMessagePage>(LIST_MESSAGES, {
    input: { view: "all", connectionId },
  });
  const rows = messages.data?.rows ?? [];
  return (
    <>
      <Toolbar
        variant="collection"
        count={
          messages.isSuccess && (
            <ListCount
              shown={rows.length}
              noun="messages"
              one="message"
              hasMore={messages.data.more}
            />
          )
        }
      >
        <RefreshControl query={messages} />
      </Toolbar>
      <MessagesGrid query={messages} rows={rows} />
    </>
  );
}
