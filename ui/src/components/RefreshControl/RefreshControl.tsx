import { RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/Button";
import { useFormatTime } from "@/lib/format";
import styles from "./RefreshControl.module.css";

/** How long a refresh shows as running at least: a local read takes milliseconds. */
const SHOWN_AT_LEAST_MS = 700;

/** The parts of an engine query the control shows and runs; any `useCommandQuery` result fits. */
interface RefreshableQuery {
  /** When the data shown was read, in milliseconds since the epoch; 0 before the first read. */
  dataUpdatedAt: number;
  isFetching: boolean;
  refetch: () => unknown;
}

/**
 * When the data shown was read ("Updated 14:03:27 CDT", announced politely when it changes)
 * and a Refresh button. While a refresh runs, for at least 0.7 seconds so a fast one is seen,
 * the button's arrows turn and the text reads Refreshing…. For shared queues that other
 * people change while the screen is open. Refreshing is on request: reads of patient data are
 * audited each time they run.
 */
export function RefreshControl({ query }: { query: RefreshableQuery }) {
  const formatTime = useFormatTime();
  const [pressed, setPressed] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const refresh = () => {
    setPressed(true);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setPressed(false), SHOWN_AT_LEAST_MS);
    void query.refetch();
  };
  const running = pressed || query.isFetching;
  const read = new Date(query.dataUpdatedAt).toISOString();
  const updated =
    query.dataUpdatedAt > 0
      ? `Updated ${formatTime(read, { today: new Date(), seconds: true })}`
      : "";
  return (
    <div className={styles.control}>
      <span role="status" className={styles.updated}>
        {running && query.dataUpdatedAt > 0 ? "Refreshing…" : updated}
      </span>
      <Button isPending={running} onPress={refresh}>
        <RefreshCw aria-hidden="true" className={styles.icon} />
        Refresh
      </Button>
    </div>
  );
}
