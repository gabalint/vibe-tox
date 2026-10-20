import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BackupRunView } from "@/generated/BackupRunView";
import type { CopyView } from "@/generated/CopyView";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import { backupKind, jobState, sizeText } from "../labels";

export function copyText(copy: CopyView): string {
  if (copy.state === "failed")
    return `${copy.targetLabel}: failed (${copy.failureCode ?? "no code"})`;
  return `${copy.targetLabel}: verified${copy.pruned ? ", since pruned" : ""}`;
}

interface RunPanelProps {
  run: BackupRunView;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One backup run in view mode, with the outcome of each copy. */
export function RunPanel({ run, ...navigation }: RunPanelProps) {
  const formatTime = useFormatTime();
  const state = jobState(run.state);
  return (
    <DetailPanel
      identifier={`${backupKind(run.kind)} backup, ${formatTime(run.requestedAt)}`}
      status={<StatusBadge tone={state.tone}>{state.label}</StatusBadge>}
      {...navigation}
    >
      <Facts
        items={[
          { label: "Requested", value: <Timestamp value={run.requestedAt} /> },
          {
            label: "Started",
            value: run.startedAt ? <Timestamp value={run.startedAt} /> : "Not started",
          },
          {
            label: "Finished",
            value: run.finishedAt ? <Timestamp value={run.finishedAt} /> : "Not finished",
          },
          { label: "Database", value: sizeText(run.databaseBytes) },
          {
            label: "Stored files",
            value: run.artifactCount === null ? "Unknown" : String(run.artifactCount),
          },
          { label: "Stored-file problems", value: String(run.artifactProblems) },
          { label: "Failure", value: run.failureCode ?? "None" },
        ]}
      />
      <h3 className={styles.subheading}>Copies</h3>
      {run.copies.length === 0 ? (
        <p>No copy was made.</p>
      ) : (
        <ul aria-label="Copies" className={styles.list}>
          {run.copies.map((copy) => (
            <li key={copy.targetId}>
              {copyText(copy)}
              {copy.verifiedAt && (
                <>
                  , <Timestamp value={copy.verifiedAt} />
                </>
              )}
            </li>
          ))}
        </ul>
      )}
    </DetailPanel>
  );
}
