import type { UseQueryResult } from "@tanstack/react-query";
import { useState } from "react";
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
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import {
  LIST_MESSAGES,
  MESSAGE_QUERIES,
  REJECT_RECEIVED,
  RESEND_DELIVERY,
  RESOLVE_REJECTION,
} from "../contract";
import { recordText } from "../labels";
import { connectionPath, UNROUTED_PATH } from "../paths";
import { type FollowUp, FollowUpDialog } from "./FollowUpDialog";
import { messageColumns } from "./messageColumns";

const ACTIONS = [
  { id: RESEND_DELIVERY, label: "Send again" },
  { id: REJECT_RECEIVED, label: "Reject…" },
  { id: RESOLVE_REJECTION, label: "Mark resolved…" },
];

interface MessagesGridProps {
  /** The log query, for its loading, error and limit states. */
  query: UseQueryResult<PartnerMessagePage, CommandError>;
  /** The rows to show: the query's rows, narrowed by the screen's filters. */
  rows: readonly PartnerMessageRow[];
  /** Whether a row opens its connection; not on the connection's own Logs tab. */
  opensConnection?: boolean;
  /** Rows checked for Export, by row ID: a check column and select all. */
  selection?: {
    selectedIds: ReadonlySet<string>;
    onChange: (selectedIds: ReadonlySet<string>) => void;
  };
}

/**
 * Messages to and from outside systems, newest first, at most 500 (REQ-INT-002, REQ-RPT-011):
 * what the transport can prove and what the other system confirmed, never a bare delivery
 * claim. A row opens its connection, or Unrouted messages for one that matched none. A message
 * whose attempts all failed can be sent again; that never makes a new message or report. A
 * received message waiting for its patient or held can be rejected with a reason, and a
 * rejection resolved with a note.
 */
export function MessagesGrid({
  query,
  rows,
  opensConnection = true,
  selection,
}: MessagesGridProps) {
  const navigate = useNavigate();
  const toast = useToast();
  const formatTime = useFormatTime();
  const person = usePersonText();
  const [followUp, setFollowUp] = useState<FollowUp | null>(null);
  const resend = useCommandMutation<unknown, { input: ResendPartnerDelivery }>(RESEND_DELIVERY, {
    invalidates: MESSAGE_QUERIES,
  });
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: PartnerMessageRow) => recordText(row),
    allowed: (row) => row.allowedActions,
    onAction: (action, row) => {
      if (action === REJECT_RECEIVED || action === RESOLVE_REJECTION) {
        setFollowUp({ action, row });
        return;
      }
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
          description:
            "Messages appear here when a report is released or a billing handoff is created for a connection.",
        }}
      >
        {() => (
          <DataGrid
            label="Messages"
            columns={messageColumns(formatTime, person)}
            rows={rows}
            getRowId={(row) => row.deliveryId ?? row.messageId}
            onRowAction={opensConnection ? open : undefined}
            rowActions={rowActions}
            // The export button stays in the filter row, counting the rows checked.
            bulk={selection && { ...selection, actions: null }}
            fill
          />
        )}
      </QueryContent>
      {followUp && <FollowUpDialog followUp={followUp} onClose={() => setFollowUp(null)} />}
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
      {/* Already on the connection: a row has nothing more to open. */}
      <MessagesGrid query={messages} rows={rows} opensConnection={false} />
    </>
  );
}
