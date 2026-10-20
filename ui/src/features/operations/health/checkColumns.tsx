import type { DataGridColumn } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import styles from "@/styles/layout.module.css";
import { Person } from "../Person";
import { HealthBadge, NONE, severityRank } from "./healthLabels";

/** The engine's sentence, then who attested the check by hand and until when, if anyone did. */
function Summary({ check }: { check: HealthCheckView }) {
  const { attestation } = check;
  return (
    <>
      <div>{check.summary}</div>
      {attestation && (
        <div className={styles.hint}>
          Attested by <Person id={attestation.by} />, <Timestamp value={attestation.at} />; valid
          until <Timestamp value={attestation.validUntil} />
        </div>
      )}
    </>
  );
}

/** The observed value, with the limit it was compared with beneath it. */
function Value({ check }: { check: HealthCheckView }) {
  return (
    <>
      <div>{check.value ?? NONE}</div>
      {check.threshold && <div className={styles.hint}>{check.threshold}</div>}
    </>
  );
}

/**
 * The engine's health checks, as it evaluated them. Sorting by state puts the most urgent
 * first. The summary and next action share the free width and wrap, so they are read in full;
 * the other columns have fixed widths, so the grid fits a 1280-pixel window. The detail code is
 * a machine code; the health history keeps it.
 */
export const CHECK_COLUMNS: readonly DataGridColumn<HealthCheckView>[] = [
  {
    id: "label",
    header: "Check",
    identifier: true,
    cell: (row) => row.label,
    sortValue: (row) => row.label,
    width: 170,
    wrap: true,
  },
  {
    id: "state",
    header: "State",
    cell: (row) => <HealthBadge state={row.state} />,
    sortValue: (row) => severityRank(row.state),
    width: 120,
  },
  { id: "summary", header: "Summary", cell: (row) => <Summary check={row} />, wrap: true },
  {
    id: "value",
    header: "Value and limit",
    cell: (row) => <Value check={row} />,
    width: 160,
    wrap: true,
  },
  {
    id: "observed",
    header: "Observed",
    cell: (row) => <Timestamp value={row.observedAt} />,
    sortValue: (row) => row.observedAt,
    width: 190,
  },
  { id: "next", header: "Next action", cell: (row) => row.nextAction ?? NONE, wrap: true },
];
