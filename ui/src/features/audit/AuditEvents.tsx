import { useMemo } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Toolbar } from "@/components/Toolbar";
import type { AuditCursor } from "@/generated/AuditCursor";
import type { AuditEventView } from "@/generated/AuditEventView";
import type { ListAuditEventsInput } from "@/generated/ListAuditEventsInput";
import { useCommandPages } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { AuditEventDetails } from "./AuditEventDetails";
import { AUDIT_COLUMNS } from "./auditColumns";
import type { AuditFilter } from "./contract";
import { streamLabel } from "./streams";

/** Events per page; the engine accepts 1 to 500. */
const PAGE_SIZE = 100;

/** Sequence numbers are per stream, so a row is its stream and sequence number. */
const getRowId = (event: AuditEventView) => `${event.stream}-${event.seq}`;

/** Where the page after `event` starts. */
const cursorAfter = ({ recordedAt, stream, seq }: AuditEventView): AuditCursor => ({
  recordedAt,
  stream,
  seq,
});

/** Events matching `filter`, newest first, with older pages loaded on request. */
export function useAuditEvents(filter: AuditFilter) {
  return useCommandPages<AuditEventView, AuditCursor>("list_audit_events", {
    key: { input: filter },
    pageArgs: (before): { input: ListAuditEventsInput } => ({
      input: { ...filter, before, limit: PAGE_SIZE },
    }),
    // A short page is the oldest one; otherwise the next page starts after its last event.
    nextCursor: (page) => {
      const last = page.at(-1);
      return page.length < PAGE_SIZE || last === undefined ? undefined : cursorAfter(last);
    },
  });
}

interface AuditEventsProps {
  filter: AuditFilter;
  events: ReturnType<typeof useAuditEvents>;
  /** Clears every filter but the stream; offered when the filters exclude every event. */
  onShowAll: () => void;
}

/**
 * The events, then "Load older events". An event opens in the side panel with all its fields
 * (`?open=<stream>-<seq>`), from the rows already read.
 */
export function AuditEvents({ filter, events, onShowAll }: AuditEventsProps) {
  const rows = useMemo(() => events.data?.pages.flat() ?? [], [events.data]);
  const record = useOpenRecord();
  const shown = filter.stream ? "this stream" : "any stream";
  const filtered =
    filter.action !== undefined ||
    filter.recordType !== undefined ||
    filter.recordId !== undefined ||
    filter.selfApprovalsOnly === true;

  if (events.isPending) return <LoadingState label="Loading audit events…" rows={8} />;
  if (events.isError && !events.isFetchNextPageError) {
    return (
      <ErrorState
        title="The audit events could not be loaded"
        error={events.error}
        onRetry={() => void events.refetch()}
      />
    );
  }
  if (rows.length === 0) {
    return filtered ? (
      <EmptyState
        title="No event matches"
        description={`No event in ${shown} matches these filters.`}
        action={<Button onPress={onShowAll}>Show all events</Button>}
      />
    ) : (
      <EmptyState title="No audit events" description={`Nothing is recorded in ${shown} yet.`} />
    );
  }
  return (
    <DetailLayout
      panel={openRecordPanel(record, rows, getRowId, (shown) => (
        <AuditEventDetails
          event={shown}
          onClose={record.close}
          onPrevious={record.previous}
          onNext={record.next}
        />
      ))}
    >
      <DataGrid
        label={filter.stream ? `${streamLabel(filter.stream)} audit events` : "Audit events"}
        columns={AUDIT_COLUMNS}
        rows={rows}
        getRowId={getRowId}
        onRowAction={(event, order) => record.open(getRowId(event), order)}
        openRowId={record.openId}
        onOrderChange={record.followOrder}
        fill
      />
      {events.isFetchNextPageError && (
        <ErrorState title="Older events could not be loaded" error={events.error} />
      )}
      {events.hasNextPage && (
        <Toolbar>
          <Button isPending={events.isFetchingNextPage} onPress={() => void events.fetchNextPage()}>
            Load older events
          </Button>
        </Toolbar>
      )}
    </DetailLayout>
  );
}
