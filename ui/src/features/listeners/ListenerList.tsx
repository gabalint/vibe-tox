import type { ReactNode } from "react";
import { Alert } from "@/components/Alert";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { ListenerRestartView } from "@/generated/ListenerRestartView";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import { useCommandQuery } from "@/lib/commands";
import { LISTENER_STATES, listenerDetailText } from "@/lib/listenerDetail";
import styles from "./listeners.module.css";

/** How often the state is asked for again while it is on screen. */
const POLL_INTERVAL_MS = 10_000;

export type ListenersQuery = ReturnType<typeof useCommandQuery<ListenerStatusView[]>>;

export function useListeners(): ListenersQuery {
  return useCommandQuery<ListenerStatusView[]>("list_listener_status", undefined, {
    refetchInterval: POLL_INTERVAL_MS,
  });
}

/** What a row shows besides the state, for people who manage analyzers (ticket #4746). */
export interface RowControls {
  /** The analyzer's name, as a link to its settings. */
  nameOf: (listener: ListenerStatusView) => ReactNode;
  /** Its Enable or Disable button. */
  actionOf: (listener: ListenerStatusView) => ReactNode;
}

/**
 * After the first start of a new version: how many listeners the app started again, and each
 * enabled one that did not start, with the reason (ADR 0020).
 */
export function RestartSummary() {
  const restart = useCommandQuery<ListenerRestartView | null>("get_listener_restart");
  const summary = restart.data;
  if (!summary || (summary.listening === 0 && summary.failed.length === 0)) return null;
  const listening = `${summary.listening} ${summary.listening === 1 ? "analyzer" : "analyzers"} listening again`;
  if (summary.failed.length === 0) {
    return (
      <Alert tone="success" title="Updated">
        {listening}.
      </Alert>
    );
  }
  return (
    <Alert tone="warning" title="Updated; some analyzers did not start listening">
      {listening}. These did not start:
      <ul>
        {summary.failed.map((listener) => (
          <li key={listener.instrumentId}>
            {listener.instrumentName}: {listenerDetailText(listener) ?? "No reason was reported."}
          </li>
        ))}
      </ul>
      Check them in Laboratory setup › Instruments.
    </Alert>
  );
}

export function ListenerList({
  listeners,
  stacked,
  controls,
}: {
  listeners: ListenersQuery;
  stacked?: boolean;
  controls?: RowControls;
}) {
  if (listeners.isPending) return <StatusBadge tone="neutral">Checking</StatusBadge>;
  // Stale states are not shown as current: a failed refresh shows "unavailable".
  if (listeners.isError) return <StatusBadge tone="neutral">Status unavailable</StatusBadge>;
  if (listeners.data.length === 0) return <span>No instruments configured</span>;
  return (
    <ul className={stacked ? styles.stacked : styles.list}>
      {listeners.data.map((listener) =>
        stacked ? (
          <ListenerRow key={listener.instrumentId} listener={listener} controls={controls} />
        ) : (
          <ListenerItem key={listener.instrumentId} listener={listener} controls={controls} />
        ),
      )}
    </ul>
  );
}

/**
 * One analyzer in the window: its name and state on the first line, why and when below in the
 * muted small text, and its Enable or Disable button at the right of both (ticket #4746).
 */
function ListenerRow({
  listener,
  controls,
}: {
  listener: ListenerStatusView;
  controls?: RowControls;
}) {
  const state = LISTENER_STATES[listener.state];
  const detail = listenerDetailText(listener);
  return (
    <li className={styles.row}>
      <span className={styles.title}>
        <span className={styles.name}>
          {controls ? controls.nameOf(listener) : listener.instrumentName}
        </span>
        <StatusBadge tone={state.tone}>{state.label}</StatusBadge>
      </span>
      <span className={styles.meta}>
        {detail && <span>{detail}</span>}
        {listener.lastActivityAt && (
          <span>
            Last activity <Timestamp value={listener.lastActivityAt} />
          </span>
        )}
      </span>
      {controls && <span className={styles.action}>{controls.actionOf(listener)}</span>}
    </li>
  );
}

function ListenerItem({
  listener,
  controls,
}: {
  listener: ListenerStatusView;
  controls?: RowControls;
}) {
  const state = LISTENER_STATES[listener.state];
  const detail = listenerDetailText(listener);
  return (
    <li className={styles.listener}>
      <span className={styles.name}>
        {controls ? controls.nameOf(listener) : listener.instrumentName}
      </span>
      <StatusBadge tone={state.tone}>{state.label}</StatusBadge>
      {listener.lastActivityAt && (
        <span className={styles.detail}>
          Last activity <Timestamp value={listener.lastActivityAt} />
        </span>
      )}
      {detail && <span className={styles.detail}>{detail}</span>}
      {controls?.actionOf(listener)}
    </li>
  );
}
