import type { ReactNode } from "react";
import { Alert } from "@/components/Alert";
import { Facts } from "@/components/Facts";
import { LoadingState } from "@/components/LoadingState";
import { AuthLayout } from "@/features/auth/AuthLayout";
import { STARTUP_STATUS } from "@/features/operations/contract";
import type { StartupStatus } from "@/generated/StartupStatus";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";

interface Guidance {
  title: string;
  steps: string;
}

/** What each startup failure means and what to do (design brief §6.4, D11). */
const GUIDANCE: Record<string, Guidance> = {
  already_open: {
    title: "The app is already open on this computer",
    steps:
      "Switch to the window that is already open. If you cannot find it, sign out of the computer, sign in again and open the app.",
  },
  data_folder_on_network: {
    title: "The data folder is on a network drive",
    steps:
      "The laboratory's database must be on a disk inside this computer: network drives can damage it. Ask your IT support to keep the app's local data folder on this computer, then open the app again.",
  },
  data_folder_unknown: {
    title: "The app could not tell whether the data folder is on this computer",
    steps:
      "The laboratory's database must be on a disk inside this computer, and the app could not confirm that the data folder's disk is one: it may be a network, virtual or unusual drive. Ask your IT support to keep the app's local data folder on an internal disk of this computer, then open the app again.",
  },
  database_unreadable: {
    title: "The database could not be opened",
    steps:
      "Do not delete or move the data folder. Contact support with the data folder shown below. If the database is damaged, it can be restored from a backup onto a new installation with the recovery kit.",
  },
  database_newer_than_app: {
    title: "The database is from a newer version of the app",
    steps:
      "This version of the app is older than the one that last used this database. Install the version that last used it, then open the app again.",
  },
  pre_migration_snapshot_failed: {
    title: "The safety copy before the database update could not be made",
    steps:
      "This version updates the database and first keeps a safety copy in the data folder. The copy failed, so the database was not updated. Check that the disk has free space, then open the app again.",
  },
  restore_apply_failed: {
    title: "The restore could not be put in place",
    steps:
      "A confirmed restore from a backup could not be completed. Do not delete or move the data folder: anything the restore replaced is kept inside it. Check that the disk has free space and that no other program is using the data folder, then open the app again; the restore continues where it stopped. If it fails again, contact support with the data folder shown below.",
  },
  installation_kind_mismatch: {
    title: "This data folder belongs to the other installation",
    steps:
      "The laboratory's installation and the demo laboratory each keep their own data folder, and neither opens the other's. Do not move or copy data folders between them. Contact support with the data folder shown below.",
  },
  demo_reset_failed: {
    title: "The demo laboratory could not be reset",
    steps:
      "Its data folder could not be deleted completely; the laboratory's own data was not touched. Close the app and open it again to retry, or leave the demo laboratory.",
  },
  restore_record_failed: {
    title: "The restore could not be recorded",
    steps:
      "A backup was put in place, but the app could not record the restore and open the recovery reconciliation, so it will not start on this data. Contact support before using this computer for laboratory work.",
  },
};

const UNKNOWN: Guidance = {
  title: "The app could not start",
  steps: "Note the time and the code below and contact support. The data folder was left in place.",
};

/**
 * Shows the app once the shell reports that the engine started; otherwise the startup-failure
 * screen (D11). If the status itself cannot be read, the app is shown and its own screens report
 * what fails.
 */
export function StartupGate({ children }: { children: ReactNode }) {
  const status = useCommandQuery<StartupStatus>(STARTUP_STATUS);
  if (status.isPending) {
    return (
      <AuthLayout>
        <LoadingState label="Starting…" />
      </AuthLayout>
    );
  }
  if (status.isSuccess && status.data.state === "failed") {
    return <StartupFailure status={status.data} />;
  }
  return children;
}

function StartupFailure({ status }: { status: StartupStatus }) {
  const guidance = (status.code && GUIDANCE[status.code]) || UNKNOWN;
  return (
    <AuthLayout>
      <div className={styles.stack}>
        <h1>{guidance.title}</h1>
        <Alert tone="danger">{guidance.steps}</Alert>
        <Facts
          items={[
            { label: "Code", value: status.code ?? "None given" },
            {
              label: "Data folder",
              value: <span className={styles.code}>{status.dataFolder ?? "Not known"}</span>,
            },
          ]}
        />
        <p className={styles.hint}>Close this window when you are done.</p>
      </div>
    </AuthLayout>
  );
}
