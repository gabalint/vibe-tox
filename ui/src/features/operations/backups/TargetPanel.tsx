import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { RecordActions } from "@/components/RowActions";
import { Timestamp } from "@/components/Timestamp";
import type { TargetView } from "@/generated/TargetView";
import { allowedActionItems } from "@/lib/allowedActions";
import { labelOf } from "@/lib/choices";
import { TARGET_KINDS, TARGET_PURPOSES } from "../labels";
import { SetsSection } from "./SetsSection";
import { TARGET_ACTIONS, type TargetDialog } from "./targetActions";
import { TargetBadge } from "./targetColumns";

interface TargetPanelProps {
  target: TargetView;
  onAction: (dialog: TargetDialog) => void;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One target in view mode: where it is, its state, and the sets the engine finds on it. */
export function TargetPanel({ target, onAction, ...navigation }: TargetPanelProps) {
  const items = allowedActionItems(TARGET_ACTIONS, target.allowedActions, onAction);
  const { lastFailure } = target;
  return (
    <DetailPanel
      identifier={target.label}
      status={<TargetBadge target={target} />}
      actions={items.length > 0 && <RecordActions items={items} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Kind", value: labelOf(TARGET_KINDS, target.kind) },
          { label: "Location", value: target.location },
          { label: "Receives", value: labelOf(TARGET_PURPOSES, target.purpose) },
          {
            label: "Last verified copy",
            value: target.lastVerifiedAt ? <Timestamp value={target.lastVerifiedAt} /> : "None",
          },
          {
            label: "Last failure",
            value: lastFailure ? (
              <>
                {lastFailure.code}, <Timestamp value={lastFailure.at} />
              </>
            ) : (
              "None"
            ),
          },
        ]}
      />
      {target.retired ? (
        <p>The copies already on a retired target are not listed.</p>
      ) : (
        <SetsSection key={target.targetId} target={target} />
      )}
    </DetailPanel>
  );
}
