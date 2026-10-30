import { Link } from "react-router";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { Tone } from "@/components/tones";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { LISTENER_STATES, listenerDetailText } from "@/lib/listenerDetail";
import { PanelGroup } from "./PanelGroup";

/** How many of the analyzer's latest imports Activity counts. */
const RECENT_IMPORTS = 50;

/**
 * The listener's state, read from the listener status the top bar already keeps fresh
 * (REQ-OPS-013). A result-file instrument's folder is "Watching" while it is read, and files
 * waiting too long need someone to act (result-files brief §6). The latest error since the app
 * started stays, with when it began, after the listener recovers.
 */
export function LiveStatus({ instrument }: { instrument: InstrumentConfiguration }) {
  const statuses = useCommandQuery<ListenerStatusView[]>("list_listener_status");
  return (
    <PanelGroup title="Live status">
      <QueryContent query={statuses} what="listener status" loadingRows={2}>
        {(rows) => {
          const status = rows.find((candidate) => candidate.instrumentId === instrument.id);
          if (status === undefined) return <p>Not started yet.</p>;
          const { tone, label } = liveState(instrument, status);
          const detail = status.state === "error" ? null : listenerDetailText(status);
          const lastError = status.lastError
            ? listenerDetailText({ detail: status.lastError })
            : null;
          return (
            <Facts
              items={[
                { label: "Status", value: <StatusBadge tone={tone}>{label}</StatusBadge> },
                {
                  label: "Last activity",
                  value:
                    status.lastActivityAt === null ? (
                      "None yet"
                    ) : (
                      <Timestamp value={status.lastActivityAt} />
                    ),
                },
                ...(detail === null ? [] : [{ label: "Detail", value: detail }]),
                ...(lastError === null || !status.lastErrorAt
                  ? []
                  : [
                      {
                        label: "Last error",
                        value: (
                          <>
                            {lastError}, <Timestamp value={status.lastErrorAt} />
                          </>
                        ),
                      },
                    ]),
              ]}
            />
          );
        }}
      </QueryContent>
    </PanelGroup>
  );
}

function liveState(
  instrument: InstrumentConfiguration,
  status: ListenerStatusView,
): { tone: Tone; label: string } {
  const state = LISTENER_STATES[status.state];
  if (instrument.connection.kind !== "result_file" || status.state === "error") return state;
  if (status.detail === "files_waiting" || status.detail === "file_too_large") {
    return { tone: "warning", label: "Watching" };
  }
  return status.state === "listening" ? { tone: "neutral", label: "Watching" } : state;
}

/**
 * The approved source results are imported from (REQ-ANA-003), or the draft waiting for
 * approval, with a link to Bench setup where sources are registered and approved.
 */
export function ResultSource({ instrument }: { instrument: InstrumentConfiguration }) {
  const sources = useCommandQuery<SourceRegistrationView[]>("list_source_registrations", {
    input: { instrumentId: instrument.id },
  });
  return (
    <PanelGroup title="Result source">
      <QueryContent query={sources} what="result sources" loadingRows={2}>
        {(rows) => {
          const active = rows.find((source) => source.state === "active");
          const shown = active ?? rows.find((source) => source.state === "draft");
          return (
            <Facts
              items={[
                ...(shown
                  ? [
                      { label: "Sender", value: `${shown.senderName} ${shown.senderVersion}` },
                      { label: "Mapped test codes", value: String(shown.testCodes.length) },
                    ]
                  : []),
                { label: "Status", value: <SourceStatus approved={active} shown={shown} /> },
                {
                  label: "Registered in",
                  value: (
                    <Link to={`/bench/setup/instruments?open=${instrument.id}`}>
                      Bench setup › Instruments
                    </Link>
                  ),
                },
              ]}
            />
          );
        }}
      </QueryContent>
    </PanelGroup>
  );
}

function SourceStatus({
  approved,
  shown,
}: {
  approved: SourceRegistrationView | undefined;
  shown: SourceRegistrationView | undefined;
}) {
  if (approved) return <StatusBadge tone="neutral">Approved</StatusBadge>;
  if (shown) return <StatusBadge tone="warning">Waiting for approval</StatusBadge>;
  return <StatusBadge tone="warning">Not registered</StatusBadge>;
}

/**
 * The analyzer's latest import and how many recent imports quarantined rows, for people who
 * review results (`review_results`, the capability of Result imports). Others see no group.
 */
export function Activity({ instrument }: { instrument: InstrumentConfiguration }) {
  const canReview = useHasCapability("review_results");
  if (!canReview) return null;
  return <ImportActivity instrument={instrument} />;
}

function ImportActivity({ instrument }: { instrument: InstrumentConfiguration }) {
  // The engine returns one row more than the limit when there are older imports.
  const attempts = useCommandQuery<ImportAttemptSummary[]>("list_import_attempts", {
    input: { instrumentId: instrument.id, limit: RECENT_IMPORTS, before: null },
  });
  const filtered = `/review/imports?instrument=${instrument.id}`;
  return (
    <PanelGroup title="Activity">
      <QueryContent query={attempts} what="imports" loadingRows={2}>
        {(page) => {
          const rows = page.slice(0, RECENT_IMPORTS);
          const latest = rows[0];
          const quarantined = rows.filter((row) => row.outcome !== "processed").length;
          const counted =
            page.length > RECENT_IMPORTS ? `of the last ${RECENT_IMPORTS}` : `of ${rows.length}`;
          return (
            <Facts
              items={[
                {
                  label: "Last import",
                  value: latest ? (
                    <Link to={`/review/imports/${latest.importAttemptId}`}>
                      <Timestamp value={latest.processedAt} />
                    </Link>
                  ) : (
                    "None yet"
                  ),
                },
                {
                  label: "Imports with quarantined rows",
                  value:
                    rows.length === 0 ? (
                      "None"
                    ) : (
                      <Link to={filtered}>{`${quarantined} ${counted}`}</Link>
                    ),
                },
              ]}
            />
          );
        }}
      </QueryContent>
    </PanelGroup>
  );
}
