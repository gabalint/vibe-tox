import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { TargetView } from "@/generated/TargetView";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { TARGET_KINDS, TARGET_PURPOSES } from "../labels";

/** Retired, available or not connected. */
export function TargetBadge({ target }: { target: TargetView }) {
  if (target.retired) return <StatusBadge tone="neutral">Retired</StatusBadge>;
  return target.available ? (
    <StatusBadge tone="neutral">Available</StatusBadge>
  ) : (
    <StatusBadge tone="warning">Not connected</StatusBadge>
  );
}

/** The state, with the last failure beneath when there was one. */
function TargetState({ target }: { target: TargetView }) {
  const { lastFailure } = target;
  return (
    <>
      <div>
        <TargetBadge target={target} />
      </div>
      {lastFailure && (
        <div className={styles.hint}>
          Last failure {lastFailure.code}, <Timestamp value={lastFailure.at} />
        </div>
      )}
    </>
  );
}

/**
 * The target's name and location wrap, so near-identical names stay distinct; the kind sits
 * above the location and the last failure under the state, so the grid fits a 1280-pixel window.
 */
export const TARGET_COLUMNS: readonly DataGridColumn<TargetView>[] = [
  {
    id: "label",
    header: "Target",
    identifier: true,
    cell: (row) => row.label,
    sortValue: (row) => row.label,
    wrap: true,
  },
  {
    id: "location",
    header: "Kind and location",
    cell: (row) => (
      <>
        <div>{labelOf(TARGET_KINDS, row.kind)}</div>
        <div className={`${styles.hint} ${styles.code}`}>{row.location}</div>
      </>
    ),
    sortValue: (row) => labelOf(TARGET_KINDS, row.kind),
    wrap: true,
  },
  {
    id: "purpose",
    header: "Receives",
    cell: (row) => labelOf(TARGET_PURPOSES, row.purpose),
    width: 170,
    wrap: true,
  },
  {
    id: "state",
    header: "State",
    cell: (row) => <TargetState target={row} />,
    width: 200,
    wrap: true,
  },
  {
    id: "verified",
    header: "Last verified copy",
    cell: (row) => (row.lastVerifiedAt ? <Timestamp value={row.lastVerifiedAt} /> : "None"),
    sortValue: (row) => row.lastVerifiedAt ?? "",
    width: 190,
  },
];
