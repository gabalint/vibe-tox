import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { Timestamp } from "@/components/Timestamp";
import type { InstalledVersionView } from "@/generated/InstalledVersionView";
import { approvalFacts } from "./ApprovalPanel";
import { Decision, VersionStatus } from "./versionColumns";

interface VersionPanelProps {
  version: InstalledVersionView;
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One installed version in view mode, with its current approval and acceptance. */
export function VersionPanel({ version, actions, ...navigation }: VersionPanelProps) {
  const { approval } = version;
  return (
    <DetailPanel
      identifier={`Version ${version.version}`}
      status={<VersionStatus status={version.status} />}
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "First started", value: <Timestamp value={version.firstStartedAt} /> },
          { label: "Last started", value: <Timestamp value={version.lastStartedAt} /> },
          { label: "Running", value: version.running ? "Yes" : "No" },
          {
            label: "Approved",
            value: (
              <Decision
                decision={
                  approval && {
                    by: approval.approvedBy,
                    byDisplay: approval.approvedByDisplay,
                    at: approval.approvedAt,
                  }
                }
              />
            ),
          },
          ...(approval ? approvalFacts(approval) : []),
          { label: "Accepted for clinical use", value: <Decision decision={version.acceptance} /> },
        ]}
      />
    </DetailPanel>
  );
}
