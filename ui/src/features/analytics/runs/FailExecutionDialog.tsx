import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { ExecutionView } from "@/generated/ExecutionView";
import type { FailExecutionInput } from "@/generated/FailExecutionInput";
import type { RunDetail } from "@/generated/RunDetail";
import type { RunPositionView } from "@/generated/RunPositionView";
import { useCommandMutation } from "@/lib/commands";
import { RUN_QUERIES } from "../contract";

interface FailExecutionDialogProps {
  position: RunPositionView;
  execution: ExecutionView;
  onClose: () => void;
}

/** Records a patient attempt as failed with the reason; a rerun links to it (REQ-ANA-001). */
export function FailExecutionDialog({ position, execution, onClose }: FailExecutionDialogProps) {
  const fail = useCommandMutation<RunDetail, { input: FailExecutionInput }>("fail_execution", {
    invalidates: RUN_QUERIES,
  });
  return (
    <ReasonConfirmDialog<FailExecutionInput>
      title={`Mark the attempt at ${position.label} failed`}
      confirmLabel="Mark failed"
      variant="danger"
      command={fail}
      input={(reason) => ({
        executionId: execution.executionId,
        expectedRevision: execution.revision,
        reason,
      })}
      successTitle={`Attempt at ${position.label} marked failed`}
      onClose={onClose}
    >
      <p>
        The attempt for {position.sampleLabel} is recorded as failed. The record stays, and a rerun
        links to it.
      </p>
    </ReasonConfirmDialog>
  );
}
