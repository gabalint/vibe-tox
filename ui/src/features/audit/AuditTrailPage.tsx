import { Link, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { isCalendarDate } from "@/lib/format";
import { AuditEvents, useAuditEvents } from "./AuditEvents";
import { AuditExportMenu, useAuditExport } from "./AuditExport";
import { AuditFilters, type CodeLabels } from "./AuditFilters";
import type { PickedRecord } from "./AuditFind";
import styles from "./audit.module.css";
import { ChainResult, useVerifyChain } from "./ChainCheck";
import { type AuditFilter, isOutcome } from "./contract";
import { isStream, streamDescription } from "./streams";

const NO_CODES: CodeLabels = [];

/**
 * The audit trail (REQ-OPS-001), for people with `view_audit`: events of every stream or of
 * one, newest first, with filters kept in the URL and applied by the engine (the action,
 * record type and user lists come from it), a check of the chosen stream's hash chain started from the page header,
 * and an export of every matching event. The engine records each view and export.
 */
export function AuditTrailPage() {
  const [params, setParams] = useSearchParams();
  const filter = filterFromParams(params);
  // Each stream has its own chain; every stream together has none to verify.
  const stream = filter.stream;
  const verify = useVerifyChain();
  const events = useAuditEvents(filter);
  const exporter = useAuditExport(filter);
  // The engine sends the actions, record types and users to offer on every event.
  const first = events.data?.pages[0]?.[0];
  const apply = (next: AuditFilter) => setParams(paramsFromFilter(next));
  // A record picked in Find also chooses its record type; clearing Find keeps the type.
  const find = (record: PickedRecord | undefined) =>
    apply({
      ...filter,
      recordType: record?.recordType ?? filter.recordType,
      recordId: record?.recordId,
    });
  const pickedLabel = filter.recordId
    ? events.data?.pages[0]?.find((event) => event.recordId === filter.recordId)?.recordLabel
    : undefined;
  return (
    <Page layout="collection">
      <PageHeader
        title="Audit trail"
        description={`${streamDescription(stream)} Newest first. Viewing this trail is recorded. ${
          stream
            ? "Verify chain checks this stream for changed or missing events."
            : "Choose a stream to verify its chain for changed or missing events."
        }`}
        actions={
          <>
            <Link to="/audit/anchors" className={styles.headerLink}>
              Audit anchors
            </Link>
            <AuditExportMenu exporter={exporter} />
            <Button
              isDisabled={!stream}
              isPending={verify.isPending}
              onPress={() => stream && verify.mutate({ input: { stream } })}
            >
              Verify chain
            </Button>
          </>
        }
      />
      {stream && <ChainResult verify={verify} stream={stream} />}
      {exporter.mutation.error && (
        <ErrorState title="Not exported" error={exporter.mutation.error} />
      )}
      <AuditFilters
        filter={filter}
        count={
          events.data &&
          (!events.isError || events.isFetchNextPageError) && (
            <ListCount
              shown={events.data.pages.reduce((total, page) => total + page.length, 0)}
              noun="events"
              one="event"
            />
          )
        }
        actions={first?.availableActions ?? NO_CODES}
        recordTypes={first?.availableRecordTypes ?? NO_CODES}
        users={first?.availableUsers ?? NO_CODES}
        pickedLabel={pickedLabel}
        onApply={apply}
        onFind={find}
      />
      <AuditEvents
        filter={filter}
        events={events}
        onShowAll={() => apply({ stream: filter.stream })}
      />
    </Page>
  );
}

function filterFromParams(params: URLSearchParams): AuditFilter {
  const stream = params.get("stream");
  const outcome = params.get("outcome");
  const day = (name: string) => {
    const value = params.get(name);
    return value !== null && isCalendarDate(value) ? value : undefined;
  };
  return {
    stream: isStream(stream) ? stream : undefined,
    action: params.get("action") || undefined,
    recordType: params.get("recordType") || undefined,
    recordId: params.get("recordId") || undefined,
    selfApprovalsOnly: params.get("selfApprovalsOnly") === "true" || undefined,
    from: day("from"),
    to: day("to"),
    by: params.get("by") || undefined,
    outcome: isOutcome(outcome) ? outcome : undefined,
  };
}

function paramsFromFilter(filter: AuditFilter): URLSearchParams {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(filter)) {
    if (value) params.set(name, String(value));
  }
  return params;
}
