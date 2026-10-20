import type { ReactNode } from "react";
import { DetailPanel } from "@/components/DetailPanel";
import type { OpenRecord } from "./openRecord";

/**
 * The `DetailLayout` panel for the record named in `?open=`, resolved against the loaded
 * rows: nothing while no record is open or the rows are still loading, `render(row)` when the
 * rows hold it, and one "Record not found" panel when they do not (an old link, a record
 * removed since, or one outside the rows loaded). `notFound` adds what this screen can say or
 * offer, such as a "Show all" button. Nothing is fetched or guessed for a missing record.
 */
export function openRecordPanel<Row>(
  record: Pick<OpenRecord, "openId" | "close">,
  rows: readonly Row[] | undefined,
  idOf: (row: Row) => string,
  render: (row: Row) => ReactNode,
  notFound?: ReactNode,
): ReactNode {
  if (record.openId === null || rows === undefined) return null;
  const open = rows.find((row) => idOf(row) === record.openId);
  if (open !== undefined) return render(open);
  return (
    <DetailPanel identifier="Record not found" onClose={record.close}>
      <p>
        This record is not in this list. It may have been removed, or it may be outside the rows
        shown.
      </p>
      {notFound}
    </DetailPanel>
  );
}
