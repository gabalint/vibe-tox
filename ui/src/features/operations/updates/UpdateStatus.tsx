import { RefreshCw } from "lucide-react";
import { useId } from "react";
import { Button } from "@/components/Button";
import { type Tone, toneIcons } from "@/components/tones";
import type { UpdateFeedView } from "@/generated/UpdateFeedView";
import { useFormatTime } from "@/lib/format";
import styles from "./AvailableUpdates.module.css";

interface UpdateStatusProps {
  /** Unknown only when neither feed could be read. */
  running?: string;
  /** The feeds that were read; a feed that could not be read is left out. */
  feeds: UpdateFeedView[];
  failed: boolean;
  refresh: { dataUpdatedAt: number; isFetching: boolean; refetch: () => unknown };
}

/**
 * The answer first: whether a newer version is published for this app, the installed version,
 * when the app last asked, and Refresh (ADR 0020). Color is paired with an icon and words, never
 * alone.
 */
export function UpdateStatus({ running, feeds, failed, refresh }: UpdateStatusProps) {
  const id = useId();
  const formatTime = useFormatTime();
  const newer = feeds.flatMap((feed) => feed.builds.filter((build) => build.newer));
  const status: { tone: Tone; title: string } =
    newer.length > 0
      ? { tone: "info", title: "An update is available" }
      : failed
        ? { tone: "warning", title: "Updates could not be checked" }
        : { tone: "success", title: "This app is up to date" };
  const Icon = toneIcons[status.tone];
  return (
    <section aria-labelledby={id} className={styles.status} data-tone={status.tone}>
      <Icon aria-hidden="true" className={styles.statusIcon} />
      <div className={styles.statusText}>
        <h2 id={id} className={styles.statusTitle}>
          {status.title}
        </h2>
        {running && (
          <p>
            Installed version: <strong>{running}</strong>
          </p>
        )}
        {refresh.dataUpdatedAt > 0 && (
          <p className={styles.meta}>
            Updated{" "}
            {formatTime(new Date(refresh.dataUpdatedAt).toISOString(), { today: new Date() })}
          </p>
        )}
      </div>
      <Button isPending={refresh.isFetching} onPress={() => void refresh.refetch()}>
        <RefreshCw aria-hidden="true" />
        Refresh
      </Button>
    </section>
  );
}
