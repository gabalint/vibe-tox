import { useQuery } from "@tanstack/react-query";
import { useRef, useState } from "react";
import { Checkbox } from "@/components/Checkbox";
import { ErrorState } from "@/components/ErrorState";
import { ListLimit, withinLimit } from "@/components/ListLimit";
import { LoadingState } from "@/components/LoadingState";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import type { CommandError } from "@/generated/CommandError";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

const REFRESH_FAILED: CommandError = {
  code: "internal",
  message: "The app could not complete the action.",
  fieldErrors: [],
  gateReasons: [],
  correlationId: "corr-0000-synthetic",
};

const FORBIDDEN: CommandError = {
  code: "forbidden",
  message: "You do not have permission for this action.",
  fieldErrors: [],
  gateReasons: [],
  correlationId: "corr-0001-synthetic",
};

/** An answer asked for with a limit of 200 plus one row: 200 are shown, and more exist. */
const PARTIAL = withinLimit(
  Array.from({ length: 201 }, (_, index) => `SYN-${index}`),
  200,
);

/** Loading, refresh and partial-list states of an engine read, on a local synthetic query. */
export function DataStateSection() {
  return (
    <GallerySection title="Data states: loading, refresh and partial lists">
      <div className={styles.stack}>
        <RefreshDemo />
        <LoadingState label="Loading synthetic worklist…" rows={4} />
        <ErrorState title="The synthetic queue could not be loaded" error={FORBIDDEN} />
        <ListLimit shown={200} end="oldest" total={{ count: 340, capped: false }} />
        <ListLimit shown={200} end="oldest" total={{ count: 500, capped: true }} />
        <ListLimit shown={PARTIAL.rows.length} end="newest" hasMore={PARTIAL.hasMore} />
      </div>
    </GallerySection>
  );
}

/**
 * A synthetic read (no engine call) whose refresh can be made to fail: the rows stay, with
 * "Could not refresh" above them.
 */
function RefreshDemo() {
  const [failRefreshes, setFailRefreshes] = useState(false);
  const reads = useRef(0);
  const query = useQuery<string[], CommandError>({
    queryKey: ["gallery-synthetic-queue"],
    queryFn: () => {
      if (failRefreshes) return Promise.reject(REFRESH_FAILED);
      reads.current += 1;
      return Promise.resolve([`SYN-${String(reads.current).padStart(6, "0")}`]);
    },
  });
  return (
    <>
      <div className={styles.row}>
        <RefreshControl query={query} />
        <Checkbox isSelected={failRefreshes} onChange={setFailRefreshes}>
          Make refreshes fail
        </Checkbox>
      </div>
      <QueryContent query={query} what="synthetic queue" loadingRows={2}>
        {(rows) => <p>Newest synthetic accession read: {rows.join(", ")}</p>}
      </QueryContent>
    </>
  );
}
