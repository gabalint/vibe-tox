import { Button } from "@/components/Button";
import { Toolbar } from "@/components/Toolbar";

interface ScanState {
  scanned: number;
  rows: readonly unknown[];
  hasNextPage: boolean;
  isFetching: boolean;
  isError: boolean;
  paused: boolean;
  scanning: boolean;
  pause: () => void;
  resume: () => void;
  loadMore: () => void;
}

/** Incomplete checked history is never presented as an empty queue or a known total. */
export function QueueScanControls({
  query,
  review = false,
}: {
  query: ScanState;
  review?: boolean;
}) {
  const incomplete = query.hasNextPage;
  const progress =
    review || query.scanned > 0
      ? `${query.scanned.toLocaleString("en-US")} candidate specimens checked · ${query.rows.length.toLocaleString("en-US")} matches loaded.`
      : `${query.rows.length.toLocaleString("en-US")} items loaded.`;
  const state = scanLabel(query);
  return (
    <Toolbar count={`${progress} ${state}`}>
      {incomplete &&
        !query.isError &&
        (query.paused ? (
          <Button onPress={query.resume}>Resume checking</Button>
        ) : query.scanning ? (
          <Button onPress={query.pause}>Pause checking</Button>
        ) : (
          <Button isPending={query.isFetching} onPress={query.loadMore}>
            Load more
          </Button>
        ))}
    </Toolbar>
  );
}

function scanLabel(query: ScanState): string {
  if (!query.hasNextPage) return "Check complete.";
  if (query.isError) return "Check incomplete. Retry the failed request to continue.";
  if (query.paused)
    return query.isFetching
      ? "Pausing after the active request; history remains unchecked."
      : "Check paused; history remains unchecked.";
  return query.scanning
    ? "Checking history; results are incomplete."
    : "More history remains unchecked.";
}
