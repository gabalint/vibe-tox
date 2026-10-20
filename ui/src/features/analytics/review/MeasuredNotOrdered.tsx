import { useState } from "react";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { MeasuredNotOrderedView } from "@/generated/MeasuredNotOrderedView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import styles from "@/styles/layout.module.css";
import { receivedValueText } from "../labels";
import { runPath } from "../paths";
import { UseMeasuredResultDialog } from "./UseMeasuredResultDialog";

/** Rows shown before "Show all": the list grows with a wide file's analytes. */
const PREVIEW = 10;

/** Measurements an add-on can use come first. */
function ordered(rows: readonly MeasuredNotOrderedView[]): MeasuredNotOrderedView[] {
  const usable = rows.filter((row) => row.addOnComponents.length > 0);
  return [...usable, ...rows.filter((row) => row.addOnComponents.length === 0)];
}

/**
 * Measurements of analytes this specimen's order did not include, kept as evidence and
 * reported nowhere (result files brief F17, Q-41). When an accepted add-on test covers the
 * analyte, a reviewer can use the measurement for it; the result then goes through technical
 * review and every release gate.
 */
export function MeasuredNotOrdered({ results }: { results: SpecimenResults }) {
  const [showAll, setShowAll] = useState(false);
  const [using, setUsing] = useState<MeasuredNotOrderedView | null>(null);
  const rows = ordered(results.measuredNotOrdered);
  if (rows.length === 0) return null;
  const canUse = results.allowedActions.includes("use_measured_result");
  const shown = showAll ? rows : rows.slice(0, PREVIEW);
  return (
    <Section title={`Measured, not ordered (${rows.length})`}>
      <p className={styles.hint}>
        Kept as evidence. No result is reported unless an accepted add-on test covers the analyte
        and a reviewer uses the measurement for it.
      </p>
      <ul aria-label="Measured, not ordered" className={styles.list}>
        {shown.map((row) => (
          <li key={row.measurementId}>
            <Toolbar>
              <span>
                {row.analyteName} ({row.analyteCode}): {receivedValueText(row)}, recorded{" "}
                <Timestamp value={row.recordedAt} />, on{" "}
                <Link to={runPath(row.runId)}>
                  {row.instrumentName}, run of <Timestamp value={row.runCreatedAt} />
                </Link>
              </span>
              {canUse && row.addOnComponents.length > 0 && (
                <Button onPress={() => setUsing(row)}>Use measured result</Button>
              )}
            </Toolbar>
          </li>
        ))}
      </ul>
      {rows.length > shown.length && (
        <div>
          <Button variant="quiet" onPress={() => setShowAll(true)}>
            Show all ({rows.length})
          </Button>
        </div>
      )}
      {using && <UseMeasuredResultDialog measurement={using} onClose={() => setUsing(null)} />}
    </Section>
  );
}
