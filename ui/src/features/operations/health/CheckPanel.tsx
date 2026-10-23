import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { Timestamp } from "@/components/Timestamp";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import { Person } from "../Person";
import { HealthBadge, NONE } from "./healthLabels";

interface CheckPanelProps {
  check: HealthCheckView;
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One health check in view mode, as the engine evaluated it, with its allowed actions. */
export function CheckPanel({ check, actions, ...navigation }: CheckPanelProps) {
  const { attestation } = check;
  return (
    <DetailPanel
      identifier={check.label}
      status={<HealthBadge state={check.state} />}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Summary", value: check.summary },
          { label: "Value", value: check.value ?? NONE },
          { label: "Limit", value: check.threshold ?? NONE },
          { label: "Observed", value: <Timestamp value={check.observedAt} /> },
          { label: "Next action", value: check.nextAction ?? NONE },
          {
            label: "Attested",
            value: attestation ? (
              <>
                By <Person id={attestation.by} display={attestation.byDisplay} />,{" "}
                <Timestamp value={attestation.at} />; valid until{" "}
                <Timestamp value={attestation.validUntil} />
              </>
            ) : (
              NONE
            ),
          },
        ]}
      />
    </DetailPanel>
  );
}
