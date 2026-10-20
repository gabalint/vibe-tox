import { useState } from "react";
import { Button } from "@/components/Button";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Toolbar } from "@/components/Toolbar";
import type { PendingAnalyteView } from "@/generated/PendingAnalyteView";
import type { RecordResultDispositionInput } from "@/generated/RecordResultDispositionInput";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { RESULT_QUERIES } from "../contract";

const analyteText = (analyte: PendingAnalyteView) =>
  `${analyte.analyteName} (${analyte.analyteCode})`;

/**
 * Ordered analytes with no result line yet (REQ-RES-001). One that will not arrive can be
 * recorded as missing with a reason; that is a result revision like any other disposition.
 */
export function PendingAnalytes({
  specimenId,
  analytes,
}: {
  specimenId: string;
  analytes: readonly PendingAnalyteView[];
}) {
  const [missing, setMissing] = useState<PendingAnalyteView | null>(null);
  if (analytes.length === 0) return null;
  return (
    <div>
      <h3 className={styles.subheading}>Waiting for results</h3>
      <ul aria-label="Waiting for results" className={styles.list}>
        {analytes.map((analyte) => (
          <li key={`${analyte.componentId}:${analyte.analyteId}`}>
            <Toolbar>
              {analyteText(analyte)}
              <Button onPress={() => setMissing(analyte)}>Record as missing</Button>
            </Toolbar>
          </li>
        ))}
      </ul>
      {missing && (
        <RecordMissingDialog
          specimenId={specimenId}
          analyte={missing}
          onClose={() => setMissing(null)}
        />
      )}
    </div>
  );
}

interface RecordMissingDialogProps {
  specimenId: string;
  analyte: PendingAnalyteView;
  onClose: () => void;
}

function RecordMissingDialog({ specimenId, analyte, onClose }: RecordMissingDialogProps) {
  const record = useCommandMutation<ResultRevisionView, { input: RecordResultDispositionInput }>(
    "record_result_disposition",
    { invalidates: RESULT_QUERIES },
  );
  return (
    <ReasonConfirmDialog
      title={`Record ${analyteText(analyte)} as missing?`}
      confirmLabel="Record as missing"
      variant="primary"
      command={record}
      input={(reason): RecordResultDispositionInput => ({
        resultId: null,
        expectedRevision: 0,
        componentId: analyte.componentId,
        analyteId: analyte.analyteId,
        specimenId,
        outcome: "missing",
        reason,
      })}
      successTitle={`${analyteText(analyte)} recorded as missing`}
      onClose={onClose}
    >
      <p>
        The analyte gets a result line recorded as missing, with your reason. It is reviewed like
        any other result line.
      </p>
    </ReasonConfirmDialog>
  );
}
