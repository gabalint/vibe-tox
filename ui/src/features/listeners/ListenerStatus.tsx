import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { Tone } from "@/components/tones";
import type { ListenerState } from "@/generated/ListenerState";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import { useCommandQuery } from "@/lib/commands";
import { listenerDetailText } from "@/lib/listenerDetail";
import styles from "./listeners.module.css";

/** How often the state is asked for again while it is on screen. */
const POLL_INTERVAL_MS = 10_000;

const STATES: Record<ListenerState, { tone: Tone; label: string }> = {
  disabled: { tone: "neutral", label: "Disabled" },
  listening: { tone: "info", label: "Listening" },
  connected: { tone: "neutral", label: "Connected" },
  receiving: { tone: "info", label: "Receiving" },
  error: { tone: "danger", label: "Error" },
  // An enabled result-file instrument without a watched folder: files are loaded by hand.
  manual_only: { tone: "neutral", label: "Manual loading only" },
};

/** The order states are counted in the top-bar summary: the ones needing attention first. */
const SUMMARY_ORDER: readonly ListenerState[] = [
  "error",
  "receiving",
  "connected",
  "listening",
  "manual_only",
  "disabled",
];

type ListenersQuery = ReturnType<typeof useCommandQuery<ListenerStatusView[]>>;

function useListeners(): ListenersQuery {
  return useCommandQuery<ListenerStatusView[]>("list_listener_status", undefined, {
    refetchInterval: POLL_INTERVAL_MS,
  });
}

/**
 * Each analyzer listener's state and last activity, on the sign-in screen (REQ-OPS-013). The
 * engine keeps listening whoever is signed in.
 */
export function ListenerStatus() {
  const listeners = useListeners();
  return (
    <section aria-label="Analyzer listeners" className={styles.listeners}>
      <span className={styles.heading}>Analyzer listeners</span>
      <ListenerList listeners={listeners} />
    </section>
  );
}

/**
 * One badge in the top bar counting the listeners by state, for example "2 analyzers:
 * 1 connected, 1 listening" (REQ-OPS-013). Pressing it shows every listener's state and last
 * activity.
 */
export function ListenerSummary() {
  const listeners = useListeners();
  const [isOpen, setOpen] = useState(false);
  const { tone, text } = summarize(listeners);
  return (
    <>
      <Button variant="quiet" onPress={() => setOpen(true)}>
        <StatusBadge tone={tone}>{text}</StatusBadge>
      </Button>
      <Dialog
        title="Analyzer listeners"
        isOpen={isOpen}
        onOpenChange={setOpen}
        actions={<Button onPress={() => setOpen(false)}>Close</Button>}
      >
        <ListenerList listeners={listeners} stacked />
      </Dialog>
    </>
  );
}

function summarize(listeners: ListenersQuery): { tone: Tone; text: string } {
  if (listeners.isPending) return { tone: "neutral", text: "Analyzers: checking" };
  if (listeners.isError) return { tone: "neutral", text: "Analyzers: status unavailable" };
  const all = listeners.data;
  if (all.length === 0) return { tone: "neutral", text: "No instruments configured" };
  const counts = SUMMARY_ORDER.flatMap((state) => {
    const count = all.filter((listener) => listener.state === state).length;
    return count === 0 ? [] : [`${count} ${STATES[state].label.toLowerCase()}`];
  });
  return {
    tone: summaryTone(new Set(all.map((listener) => listener.state))),
    text: `${all.length} ${all.length === 1 ? "analyzer" : "analyzers"}: ${counts.join(", ")}`,
  };
}

/** The tone of the state needing the most attention: an error, then one still waiting. */
function summaryTone(states: Set<ListenerState>): Tone {
  if (states.has("error")) return "danger";
  if (states.has("listening")) return "info";
  if (states.has("connected") || states.has("receiving")) return "neutral";
  return "neutral";
}

function ListenerList({ listeners, stacked }: { listeners: ListenersQuery; stacked?: boolean }) {
  if (listeners.isPending) return <StatusBadge tone="neutral">Checking</StatusBadge>;
  // Stale states are not shown as current: a failed refresh shows "unavailable".
  if (listeners.isError) return <StatusBadge tone="neutral">Status unavailable</StatusBadge>;
  if (listeners.data.length === 0) return <span>No instruments configured</span>;
  return (
    <ul className={stacked ? styles.stacked : styles.list}>
      {listeners.data.map((listener) => (
        <ListenerItem key={listener.instrumentId} listener={listener} />
      ))}
    </ul>
  );
}

function ListenerItem({ listener }: { listener: ListenerStatusView }) {
  const state = STATES[listener.state];
  const detail = listenerDetailText(listener);
  return (
    <li className={styles.listener}>
      <span className={styles.name}>{listener.instrumentName}</span>
      <StatusBadge tone={state.tone}>{state.label}</StatusBadge>
      {listener.lastActivityAt && (
        <span className={styles.detail}>
          Last activity <Timestamp value={listener.lastActivityAt} />
        </span>
      )}
      {detail && <span className={styles.detail}>{detail}</span>}
    </li>
  );
}
