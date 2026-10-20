import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { BackupStatus } from "@/generated/BackupStatus";
import { backupKind, jobState } from "../labels";
import { LatestRestoreTest, LatestRun } from "./LatestRows";

/** Current protection state and the latest outcomes before the backup history tabs. */
export function StatusBand({ status }: { status: BackupStatus }) {
  const { newestVerified, running, nextDueAt } = status;
  return (
    <Facts
      layout="compact"
      items={[
        {
          label: "Data",
          value: status.upToDate ? (
            <StatusBadge tone="neutral">Every change is in a verified copy</StatusBadge>
          ) : (
            <StatusBadge tone="warning">Changes not yet in a verified copy</StatusBadge>
          ),
        },
        {
          label: "Newest verified copy",
          value: newestVerified ? (
            <>
              <Timestamp value={newestVerified.at} /> on {newestVerified.targetLabel}
            </>
          ) : (
            "None yet"
          ),
        },
        {
          label: "Running now",
          value: running ? (
            <>
              {backupKind(running.kind)} backup, {jobState(running.state).label.toLowerCase()} since{" "}
              <Timestamp value={running.startedAt ?? running.requestedAt} />
            </>
          ) : (
            "Nothing"
          ),
        },
        {
          label: "Next backup due",
          value: nextDueAt ? <Timestamp value={nextDueAt} /> : "Not scheduled",
        },
        { label: "Latest run", value: <LatestRun /> },
        { label: "Latest restore test", value: <LatestRestoreTest /> },
      ]}
    />
  );
}
