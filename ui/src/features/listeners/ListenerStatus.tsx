import { useState } from "react";
import { Button } from "@/components/Button";
import { StatusBadge } from "@/components/StatusBadge";
import type { Tone } from "@/components/tones";
import type { ListenerState } from "@/generated/ListenerState";
import { LISTENER_STATES } from "@/lib/listenerDetail";
import { ListenerList, type ListenersQuery, RestartSummary, useListeners } from "./ListenerList";
import { ListenersDialog } from "./ListenersDialog";
import styles from "./listeners.module.css";

/** The order states are counted in the top-bar summary: the ones needing attention first. */
const SUMMARY_ORDER: readonly ListenerState[] = [
  "error",
  "receiving",
  "connected",
  "listening",
  "manual_only",
  "disabled",
];

/**
 * Each analyzer listener's state and last activity, on the sign-in screen (REQ-OPS-013). The
 * engine keeps listening whoever is signed in.
 */
export function ListenerStatus() {
  const listeners = useListeners();
  return (
    <section aria-label="Analyzer listeners" className={styles.listeners}>
      <span className={styles.heading}>Analyzer listeners</span>
      <RestartSummary />
      <ListenerList listeners={listeners} />
    </section>
  );
}

/**
 * One badge in the top bar counting the listeners by state, for example "2 analyzers:
 * 1 connected, 1 listening" (REQ-OPS-013). Pressing it shows every listener's state and last
 * activity, after an update with the restart summary; people who manage analyzers enable and
 * disable them there (ticket #4746).
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
      {isOpen && <ListenersDialog listeners={listeners} onClose={() => setOpen(false)} />}
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
    return count === 0 ? [] : [`${count} ${LISTENER_STATES[state].label.toLowerCase()}`];
  });
  return {
    tone: summaryTone(new Set(all.map((listener) => listener.state))),
    text: `${all.length} ${all.length === 1 ? "analyzer" : "analyzers"}: ${counts.join(", ")}`,
  };
}

/** The tone of the state needing the most attention: an error, then any working listener. */
function summaryTone(states: Set<ListenerState>): Tone {
  if (states.has("error")) return "danger";
  if (states.has("listening") || states.has("connected") || states.has("receiving")) return "info";
  return "neutral";
}
