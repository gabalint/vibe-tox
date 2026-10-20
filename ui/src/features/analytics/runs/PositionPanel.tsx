import { Button } from "@/components/Button";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import type { LotView } from "@/generated/LotView";
import type { RunPositionView } from "@/generated/RunPositionView";
import { labelOf } from "@/lib/choices";
import { IdentityBanner } from "@/lib/IdentityBanner";
import styles from "@/styles/layout.module.css";
import { ATTEMPT_KINDS, lotText, POSITION_ROLES } from "../labels";
import { ExecutionState } from "./ExecutionState";

interface PositionPanelProps {
  position: RunPositionView;
  /** The run's lots, to name a control or calibrator lot. */
  lots: readonly LotView[];
  /** The same actions as the position's row menu. */
  actions: readonly ActionItem[];
  onRepeat?: () => void;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One run position: what was placed there and, for patient material, its attempt. */
export function PositionPanel({
  position,
  lots,
  actions,
  onRepeat,
  ...navigation
}: PositionPanelProps) {
  const { execution } = position;
  const lot = lots.find((candidate) => candidate.lotId === position.lotId);
  return (
    <DetailPanel
      identifier={`Position ${position.label}`}
      actions={
        <>
          {onRepeat && <Button onPress={onRepeat}>Repeat test</Button>}
          {actions.length > 0 && <RecordActions items={actions} />}
        </>
      }
      {...navigation}
    >
      {position.patientContext && (
        <IdentityBanner
          subject={position.patientContext.subject}
          specimen={{
            accessionNumber: position.patientContext.accessionNumber,
            labelCode: position.sampleLabel,
          }}
        />
      )}
      <Facts
        items={[
          ...(position.patientContext
            ? [{ label: "Specimen type", value: position.patientContext.specimenTypeCode }]
            : []),
          { label: "Role", value: labelOf(POSITION_ROLES, position.role) },
          {
            label: "Sample label",
            value: <span className={styles.code}>{position.sampleLabel}</span>,
          },
          ...(lot ? [{ label: "Lot", value: lotText(lot) }] : []),
          ...(execution
            ? [
                { label: "Attempt", value: labelOf(ATTEMPT_KINDS, execution.attemptKind) },
                { label: "Execution", value: <ExecutionState execution={execution} /> },
                { label: "Reason for the attempt", value: execution.reason ?? "Not recorded" },
                { label: "Dilution factor", value: execution.dilutionFactor ?? "None" },
                {
                  label: "Preparation batch",
                  value:
                    execution.prepBatchId ??
                    "None linked; repeat with the correct preparation if needed",
                },
                { label: "Expected analytes", value: execution.expectedAnalytes },
              ]
            : []),
        ]}
      />
    </DetailPanel>
  );
}
