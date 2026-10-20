import { Link } from "react-router";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { HandoffItemView } from "@/generated/HandoffItemView";
import type { HandoffOutcomeView } from "@/generated/HandoffOutcomeView";
import { labelOf } from "@/lib/choices";
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import { ITEM_KINDS, itemStatus, OUTCOMES } from "../labels";
import { derivationPath } from "../paths";

/** How an item is named: by its derivation, which the handoff holds once. */
export const derivationText = (item: HandoffItemView) => `Derivation ${item.derivationNo}`;

interface HandoffItemPanelProps {
  item: HandoffItemView;
  /** The same actions as the item's row menu, from its `allowedActions`. */
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * One handoff item in view mode (REQ-BIL-002): its derivation, which it links to, what it
 * adjusts, and the billing partner's outcome as recorded, with who recorded it.
 */
export function HandoffItemPanel({ item, actions, ...navigation }: HandoffItemPanelProps) {
  const status = itemStatus(item.status);
  return (
    <DetailPanel
      identifier={derivationText(item)}
      status={<StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          {
            label: "Derivation",
            value: <Link to={derivationPath(item.derivationId)}>{derivationText(item)}</Link>,
          },
          { label: "Item", value: <span className={styles.code}>{item.itemId}</span> },
          { label: "Kind", value: labelOf(ITEM_KINDS, item.kind) },
          { label: "Adjusts item", value: item.adjustsItemId ?? "None" },
          {
            label: "Outcome",
            value: item.outcome ? <OutcomeText outcome={item.outcome} /> : "None recorded yet",
          },
          ...(item.outcomeHistory && item.outcomeHistory.length > 1
            ? [
                {
                  label: "Outcome history",
                  value: (
                    <ol className={styles.list}>
                      {item.outcomeHistory.map((outcome, index) => (
                        // biome-ignore lint/suspicious/noArrayIndexKey: The engine's outcome history is append-only and never reordered.
                        <li key={`${outcome.recordedAt}-${index}`}>
                          <OutcomeText outcome={outcome} />
                        </li>
                      ))}
                    </ol>
                  ),
                },
              ]
            : []),
        ]}
      />
    </DetailPanel>
  );
}

function OutcomeText({ outcome }: { outcome: HandoffOutcomeView }) {
  const person = usePersonText();
  const detail = outcome.reason ?? outcome.externalReference;
  return (
    <>
      {labelOf(OUTCOMES, outcome.outcome)}
      {detail && `: ${detail}`}. Recorded <Timestamp value={outcome.recordedAt} /> by{" "}
      {person(outcome.recordedBy, outcome.recordedByDisplay)}.
    </>
  );
}
