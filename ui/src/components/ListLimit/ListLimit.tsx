import styles from "./ListLimit.module.css";

interface ListLimitProps {
  /** How many rows are shown. */
  shown: number;
  /** Which rows of the engine's order are shown. */
  end: "oldest" | "newest" | "first";
  /** The engine's count, when its answer has one; `capped` when the engine stopped counting. */
  total?: { count: number; capped: boolean };
  /** Without a count: whether more rows exist (see `withinLimit`). */
  hasMore?: boolean;
  /** The toolbar already holds the count; retain only the order and subset explanation. */
  variant?: "summary";
}

/**
 * The rows of an answer asked for with `limit + 1`: the first `limit` to show, and whether
 * more exist. The extra row only says that the list goes on; it is never shown.
 */
export function withinLimit<Row>(
  rows: readonly Row[],
  limit: number,
): { rows: readonly Row[]; hasMore: boolean } {
  return { rows: rows.slice(0, limit), hasMore: rows.length > limit };
}

/**
 * The line under a list that the engine answered only in part: how many rows are shown, of
 * how many, and what to do to see the rest. Shows nothing when every row is shown.
 */
export function ListLimit({ shown, end, total, hasMore = false, variant }: ListLimitProps) {
  const more = total === undefined ? hasMore : total.count > shown || total.capped;
  if (!more) return null;
  if (variant === "summary") {
    return (
      <p className={styles.limit}>
        Showing the {end} results; sorting a column orders only the rows shown.
      </p>
    );
  }
  let of = "";
  if (total !== undefined) of = total.capped ? ` of ${total.count} or more` : ` of ${total.count}`;
  return (
    <p className={styles.limit}>
      Showing the {end} {shown}
      {of}. Narrow the filter to see the rest; sorting a column orders only the rows shown.
    </p>
  );
}

/** Counts for bounded responses. Only engine evidence can establish a total. */
export function ListCount({
  shown,
  noun,
  one,
  total,
  hasMore = false,
}: Omit<ListLimitProps, "end" | "variant"> & { noun: string; one: string }) {
  const number = shown.toLocaleString("en-US");
  if (total?.capped) {
    return `${number} shown · ${total.count.toLocaleString("en-US")} or more ${noun}`;
  }
  if (total) {
    const label = total.count === 1 ? one : noun;
    return shown === total.count
      ? `${number} ${label}`
      : `${number} of ${total.count.toLocaleString("en-US")} ${label}`;
  }
  return `${number} ${shown === 1 ? one : noun} shown${hasMore ? " · more available" : ""}`;
}
