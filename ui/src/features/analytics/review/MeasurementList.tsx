import { StatusBadge } from "@/components/StatusBadge";
import type { MeasurementView } from "@/generated/MeasurementView";
import styles from "@/styles/layout.module.css";
import { measurementSource, measurementText } from "../resultLabels";

/** Source measurements as the analyzer reported them, each with where it came from. */
export function MeasurementList({
  label,
  measurements,
}: {
  label: string;
  measurements: readonly MeasurementView[];
}) {
  return (
    <ul aria-label={label} className={styles.list}>
      {measurements.map((measurement) => (
        <li key={measurement.measurementId}>
          {measurementText(measurement)}. {measurementSource(measurement)}
          {measurement.superseded && (
            <>
              {" "}
              <StatusBadge tone="neutral">Replaced by a later export</StatusBadge>
            </>
          )}
        </li>
      ))}
    </ul>
  );
}
