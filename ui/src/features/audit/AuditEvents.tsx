import { useMemo } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Toolbar } from "@/components/Toolbar";
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

const getRowId = (event: AuditEventView) => String(event.seq);

/** Events matching `filter`, newest first, with older pages loaded on request. */
export function useAuditEvents(filter: AuditFilter) {
  return useCommandPages<AuditEventView>("list_audit_events", {
    key: { input: filter },
    pageArgs: (beforeSeq): { input: ListAuditEventsInput } => ({
      input: { ...filter, beforeSeq, limit: PAGE_SIZE },
    }),
    // A short page is the oldest one; otherwise the next page starts before its last event.
    nextCursor: (page) => (page.length < PAGE_SIZE ? undefined : page.at(-1)?.seq),
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
 * (`?open=<seq>`), from the rows already read.
 */
export function AuditEvents({ filter, events, onShowAll }: AuditEventsProps) {
  const rows = useMemo(() => events.data?.pages.flat() ?? [], [events.data]);
  const record = useOpenRecord();
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
        description="No event in this stream matches these filters."
        action={<Button onPress={onShowAll}>Show all events</Button>}
      />
    ) : (
      <EmptyState title="No audit events" description="Nothing is recorded in this stream yet." />
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
        label={`${streamLabel(filter.stream)} audit events`}
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
