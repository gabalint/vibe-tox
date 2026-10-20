import { useId, useState } from "react";
import { Button } from "@/components/Button";
import { GateReasons } from "@/components/ErrorState";
import { Inset } from "@/components/Inset";
import { StatusBadge } from "@/components/StatusBadge";
import { Toolbar } from "@/components/Toolbar";
import type { GateReason } from "@/generated/GateReason";
import type { ResultLineView } from "@/generated/ResultLineView";
import styles from "@/styles/layout.module.css";
import { reviewStatus } from "../resultLabels";
import { DispositionDialog } from "./DispositionDialog";
import { MeasurementList } from "./MeasurementList";
import { RevisionFacts } from "./RevisionFacts";
import { analyteText } from "./resultColumns";
import { SelectEvidenceDialog } from "./SelectEvidenceDialog";

type OpenDialog = "evidence" | "disposition" | null;

/**
 * The open result line's detail (REQ-RES-001): the current revision with its evidence, the
 * gates that block its review, why an earlier review is out of date, measurements not
 * selected, earlier revisions, and the line's own actions.
 */
export function ResultLineDetail({ line }: { line: ResultLineView }) {
  const [dialog, setDialog] = useState<OpenDialog>(null);
  const headingId = useId();
  const close = () => setDialog(null);
  const analyte = analyteText(line);
  const status = reviewStatus(line.reviewStatus);
  const earlier = line.history.filter((item) => item.revisionId !== line.current?.revisionId);
  return (
    <section aria-labelledby={headingId}>
      <Inset>
        <Toolbar>
          <h3 id={headingId} className={styles.subheading}>
            {analyte}
          </h3>
          {line.purpose === "validity" && <StatusBadge tone="info">Specimen validity</StatusBadge>}
          <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
        </Toolbar>
        <Toolbar>
          {line.unselectedEvidence.length > 0 && (
            <Button onPress={() => setDialog("evidence")}>Select evidence</Button>
          )}
          <Button onPress={() => setDialog("disposition")}>Record disposition</Button>
        </Toolbar>
        {line.current ? (
          <RevisionFacts revision={line.current} analyte={analyte} />
        ) : (
          <p>No current result.</p>
        )}
        <GateList label={`Blocking review of ${analyte}`} gates={line.gates} />
        {line.reviewStatus.status === "stale" && (
          <GateList
            label={`Why the review of ${analyte} is out of date`}
            gates={line.reviewStatus.reasons}
          />
        )}
        {line.unselectedEvidence.length > 0 && (
          <div>
            <h4 className={styles.subheading}>Measurements not selected</h4>
            <MeasurementList
              label={`Measurements not selected for ${analyte}`}
              measurements={line.unselectedEvidence}
            />
          </div>
        )}
        {earlier.length > 0 && (
          <details>
            <summary>Earlier revisions ({earlier.length})</summary>
            <ul className={styles.list}>
              {earlier.map((revision) => (
                <li key={revision.revisionId}>
                  <RevisionFacts revision={revision} analyte={analyte} />
                </li>
              ))}
            </ul>
          </details>
        )}
      </Inset>
      {dialog === "evidence" && <SelectEvidenceDialog line={line} onClose={close} />}
      {dialog === "disposition" && <DispositionDialog line={line} onClose={close} />}
    </section>
  );
}

/** Gate reasons, labelled, with the engine's words; nothing is listed when there are none. */
function GateList({ label, gates }: { label: string; gates: readonly GateReason[] }) {
  if (gates.length === 0) return null;
  return (
    <div>
      <h4 className={styles.subheading}>{label}</h4>
      <GateReasons label={label} gates={gates} />
    </div>
  );
}
