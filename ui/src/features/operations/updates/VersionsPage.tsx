import { useState } from "react";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Tabs } from "@/components/Tabs";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { MANAGE_CONFIGURATION } from "../contract";
import { ApprovalsSection } from "./ApprovalsSection";
import { AvailableUpdates } from "./AvailableUpdates";
import { InstalledVersions } from "./InstalledVersions";
import { RecordApprovalDialog } from "./RecordApprovalDialog";
import { RequestUpdateBackup } from "./RequestUpdateBackup";

const PATH = "/operations/versions";

const TABS = [
  { id: "installed", label: "Installed versions" },
  { id: "available", label: "Available updates" },
  { id: "approvals", label: "Update approvals" },
] as const;

/**
 * Installed versions and their update records (REQ-OPS-006, NFR-24; design brief §7): the
 * approval before an update, the verified pre-update backup, and acceptance for clinical use
 * of the version that now runs. Available updates are checked, downloaded and installed from
 * the app (ADR 0020). The approvals tab is for people with `manage_configuration`; the base
 * path shows the installed versions.
 */
export function VersionsPage() {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const canApprove = session.data?.capabilities.includes(MANAGE_CONFIGURATION) ?? false;
  const [approving, setApproving] = useState(false);
  const routeTabs = useRouteTabs(PATH, TABS);
  const tabs = canApprove ? routeTabs.tabs : routeTabs.tabs.filter(({ id }) => id !== "approvals");
  const tab =
    routeTabs.selectedId === "approvals" && !canApprove ? "installed" : routeTabs.selectedId;
  return (
    <Page>
      <PageHeader
        title="Versions and updates"
        actions={
          canApprove && <Button onPress={() => setApproving(true)}>Record update approval</Button>
        }
        description="Updates published for this app, and records of each update: its approval, the backup taken before it, and acceptance for clinical use."
      />
      {tab === "approvals" && canApprove && (
        <Section title="Before installing a new version">
          <p>
            Review the downloaded release and use a verified manual or pre-update backup from the
            last 24 hours. Record update approval before installing the package.
          </p>
          <RequestUpdateBackup />
        </Section>
      )}
      <QueryContent query={session} what="session">
        {() => (
          <Tabs
            contentLayout="collection"
            label="Versions and updates"
            tabs={tabs}
            selectedId={tab}
          >
            {tab === "installed" && <InstalledVersions />}
            {tab === "available" && <AvailableUpdates />}
            {tab === "approvals" && <ApprovalsSection />}
          </Tabs>
        )}
      </QueryContent>
      {approving && <RecordApprovalDialog onClose={() => setApproving(false)} />}
    </Page>
  );
}
