import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { RestoreTestView } from "@/generated/RestoreTestView";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import { AnchorCounts } from "../AnchorCounts";
import { findingText, jobState } from "../labels";

export function TestState({ test }: { test: RestoreTestView }) {
  const state = jobState(test.state);
  return <StatusBadge tone={state.tone}>{state.label}</StatusBadge>;
}

/** The restore time; a test still waiting or running has none yet. */
export function elapsedText(elapsedMs: number | null): string {
  return elapsedMs === null ? "Not finished" : `${(elapsedMs / 1000).toFixed(1)} s`;
}

interface RestoreTestPanelProps {
  test: RestoreTestView;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One restore test in view mode: how it went, its findings and the anchor comparison. */
export function RestoreTestPanel({ test, ...navigation }: RestoreTestPanelProps) {
  const formatTime = useFormatTime();
  return (
    <DetailPanel
      identifier={`Restore test of ${test.targetLabel}, ${formatTime(test.requestedAt)}`}
      status={<TestState test={test} />}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Requested", value: <Timestamp value={test.requestedAt} /> },
          {
            label: "Started",
            value: test.startedAt ? <Timestamp value={test.startedAt} /> : "Not started",
          },
          {
            label: "Finished",
            value: test.finishedAt ? <Timestamp value={test.finishedAt} /> : "Not finished",
          },
          { label: "Restore time", value: elapsedText(test.elapsedMs) },
          { label: "Backup set", value: test.setId ?? "The newest set, chosen when the test runs" },
          { label: "Failure", value: test.failureCode ?? "None" },
        ]}
      />
      {test.finishedAt === null ? null : test.findings.length === 0 ? (
        <p>No findings.</p>
      ) : (
        <ul aria-label="Findings" className={styles.list}>
          {test.findings.map((finding) => (
            <li key={finding.code}>
              {findingText(finding.code)}: {finding.count}
            </li>
          ))}
        </ul>
      )}
      {test.finishedAt !== null && <AnchorCounts summary={test.anchors} />}
    </DetailPanel>
  );
}
