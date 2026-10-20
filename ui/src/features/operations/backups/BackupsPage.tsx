import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Tabs } from "@/components/Tabs";
import { useToast } from "@/components/Toast";
import type { BackupRunView } from "@/generated/BackupRunView";
import type { BackupStatus } from "@/generated/BackupStatus";
import type { RequestBackupInput } from "@/generated/RequestBackupInput";
import { useCommandMutation } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { BACKUP_QUERIES, GET_BACKUP_STATUS, REQUEST_BACKUP } from "../contract";
import { useJobQuery } from "../useJobQuery";
import { RestoreTestsSection } from "./RestoreTestsSection";
import { RunsSection } from "./RunsSection";
import { StatusBand } from "./StatusBand";
import { TargetsSection } from "./TargetsSection";
import { TAB_PATH } from "./tabPath";

const TABS = [
  { id: "targets", label: "Targets" },
  { id: "runs", label: "Runs" },
  { id: "restore-tests", label: "Restore tests" },
] as const;

/**
 * Backups (REQ-OPS-007, NFR-04, NFR-20, NFR-30): encrypted copies of the laboratory's data in
 * the folders the laboratory chose, the runs that made them, and restore tests that prove a copy
 * can be read back. A copy counts only once the engine has verified it. An overview: the
 * protection state first, then the targets, runs and restore tests as tabs.
 */
export function BackupsPage() {
  const toast = useToast();
  const status = useJobQuery<BackupStatus>(
    GET_BACKUP_STATUS,
    undefined,
    (data) => data.running !== null,
  );
  const request = useCommandMutation<BackupRunView, { input: RequestBackupInput }>(REQUEST_BACKUP, {
    invalidates: BACKUP_QUERIES,
  });
  const backUpNow = () =>
    request.mutate(
      { input: { kind: "manual" } },
      {
        onSuccess: () =>
          toast.show({
            tone: "info",
            title: "Backup requested",
            description: "It counts once the copy is verified; the Runs tab shows its progress.",
          }),
      },
    );
  const { selectedId: tab, tabs } = useRouteTabs(TAB_PATH, TABS);
  return (
    <Page>
      <PageHeader
        title="Backups"
        actions={
          <Button variant="primary" isPending={request.isPending} onPress={backUpNow}>
            Back up now
          </Button>
        }
        description="Encrypted copies of the laboratory's data in the folders you choose, and restore tests that prove a copy can be read back."
      />
      <QueryContent query={status} what="backup status">
        {(data) => <StatusBand status={data} />}
      </QueryContent>
      {request.error && <ErrorState title="Backup not requested" error={request.error} />}
      <Tabs contentLayout="collection" label="Backups" tabs={tabs} selectedId={tab}>
        {tab === "targets" && <TargetsSection />}
        {tab === "runs" && <RunsSection />}
        {tab === "restore-tests" && <RestoreTestsSection />}
      </Tabs>
    </Page>
  );
}
