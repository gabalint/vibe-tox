import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { AppRunView } from "@/generated/AppRunView";
import { useFormatTime } from "@/lib/format";
import { Person } from "../Person";
import { NONE, stopKindText } from "./healthLabels";

export function gapText(run: AppRunView): string {
  const parts = [
    run.gapSeconds === null ? null : `${run.gapSeconds} seconds before this start`,
    run.clockGapSeconds === null
      ? null
      : `${run.clockGapSeconds} seconds when the clock jumped ahead during this run`,
  ].filter((part) => part !== null);
  return `No analyzer results were received for ${parts.join(" and ")}. Note what you checked, such as results to send again from the analyzers.`;
}

const seconds = (value: number | null) => (value === null ? NONE : `${value} s`);

interface AppRunPanelProps {
  run: AppRunView;
  actions: readonly ActionItem[];
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One run of the app in view mode, with its listening gaps and who acknowledged them. */
export function AppRunPanel({ run, actions, ...navigation }: AppRunPanelProps) {
  const formatTime = useFormatTime();
  const { acknowledged } = run;
  return (
    <DetailPanel
      identifier={`Run started ${formatTime(run.startedAt)}`}
      status={
        run.needsAcknowledgement && <StatusBadge tone="warning">Gap not acknowledged</StatusBadge>
      }
      actions={actions.length > 0 && <RecordActions items={actions} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Started", value: <Timestamp value={run.startedAt} /> },
          {
            label: "Stopped",
            value: run.stoppedAt ? <Timestamp value={run.stoppedAt} /> : NONE,
          },
          { label: "How it stopped", value: stopKindText(run.stopKind) },
          { label: "App version", value: run.appVersion },
          { label: "Web engine", value: run.webview ?? NONE },
          { label: "Gap before start", value: seconds(run.gapSeconds) },
          { label: "Clock gaps", value: seconds(run.clockGapSeconds) },
          { label: "Previous stop unclean", value: run.previousUnclean ? "Yes" : "No" },
          {
            label: "Gaps acknowledged",
            value: acknowledged ? (
              <>
                <Person id={acknowledged.by} />, <Timestamp value={acknowledged.at} />
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
