import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { ActionItem } from "@/components/RowActions";
import { RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { AssessmentHistory } from "../AssessmentHistory";
import { QcAssessment } from "./QcAssessment";
import { currentObservations, type QcScope, scopeStatus } from "./qcScopes";

interface ScopePanelProps {
  scope: QcScope;
  /** The same actions as the scope's row menu. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * One scope's current and earlier assessments. An assessment is never edited: a new one
 * replaces it with a reason, and a rejection cannot be replaced by an acceptance.
 */
export function ScopePanel({ scope, actions, ...navigation }: ScopePanelProps) {
  const { assessments } = scope;
  const status = scopeStatus(scope);
  return (
    <DetailPanel
      identifier={scope.title}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={<RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[{ label: "Current observations", value: currentObservations(scope).length }]}
      />
      <AssessmentHistory
        current={assessments.current}
        history={assessments.history}
        getId={(item) => item.assessmentId}
        render={(item) => <QcAssessment item={item} />}
        emptyText="Not assessed yet."
      />
    </DetailPanel>
  );
}
