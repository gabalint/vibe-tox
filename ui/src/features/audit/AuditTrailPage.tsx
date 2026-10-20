import { Link, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { AuditEvents, useAuditEvents } from "./AuditEvents";
import { AuditFilters, type CodeLabels } from "./AuditFilters";
import styles from "./audit.module.css";
import { ChainResult, useVerifyChain } from "./ChainCheck";
import type { AuditFilter } from "./contract";
import { isStream } from "./streams";

const NO_CODES: CodeLabels = [];

/**
 * The audit trail (REQ-OPS-001), for people with `view_audit`: events newest first, with
 * filters kept in the URL (the action and record type lists come from the engine), and a check of the stream's hash chain started from the page
 * header. The engine records each view as an access event.
 */
export function AuditTrailPage() {
  const [params, setParams] = useSearchParams();
  const filter = filterFromParams(params);
  const verify = useVerifyChain();
  const events = useAuditEvents(filter);
  // The engine sends the stream's actions and record types on every event.
  const first = events.data?.pages[0]?.[0];
  const apply = (next: AuditFilter) => setParams(paramsFromFilter(next));
  // A find replaces the address rather than adding an entry per pause in typing.
  const find = (recordId: string | undefined) =>
    setParams(paramsFromFilter({ ...filter, recordId }), { replace: true });
  return (
    <Page layout="collection">
      <PageHeader
        title="Audit trail"
        description="Newest first. Viewing this trail is recorded. Verify chain checks for changed or missing events."
        actions={
          <>
            <Link to="/audit/anchors" className={styles.headerLink}>
              Audit anchors
            </Link>
            <Button
              isPending={verify.isPending}
              onPress={() => verify.mutate({ input: { stream: filter.stream } })}
            >
              Verify chain
            </Button>
          </>
        }
      />
      <ChainResult verify={verify} stream={filter.stream} />
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
  return {
    stream: isStream(stream) ? stream : "security",
    action: params.get("action") || undefined,
    recordType: params.get("recordType") || undefined,
    recordId: params.get("recordId") || undefined,
    selfApprovalsOnly: params.get("selfApprovalsOnly") === "true" || undefined,
  };
}

function paramsFromFilter(filter: AuditFilter): URLSearchParams {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(filter)) {
    if (value) params.set(name, String(value));
  }
  return params;
}
