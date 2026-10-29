import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Tabs } from "@/components/Tabs";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { MANAGE_CONFIGURATION } from "../contract";
import { ApprovalsSection } from "./ApprovalsSection";
import { AvailableUpdates } from "./AvailableUpdates";
import { InstalledVersions } from "./InstalledVersions";

const PATH = "/operations/versions";

const TABS = [
  { id: "available", label: "Available updates" },
  { id: "installed", label: "Installed versions" },
  { id: "approvals", label: "Update approvals" },
] as const;

/**
 * Installed versions and their update records (REQ-OPS-006, NFR-24; design brief §7): the
 * approval before an update, the verified pre-update backup, and acceptance for clinical use
 * of the version that now runs. Available updates are checked, downloaded and installed from
 * the app (ADR 0020). The approvals tab is for people with `manage_configuration`; it also
 * approves a package obtained outside the app. The base path shows the available updates.
 */
export function VersionsPage() {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const canApprove = session.data?.capabilities.includes(MANAGE_CONFIGURATION) ?? false;
  const routeTabs = useRouteTabs(PATH, TABS);
  const tabs = canApprove ? routeTabs.tabs : routeTabs.tabs.filter(({ id }) => id !== "approvals");
  const tab =
    routeTabs.selectedId === "approvals" && !canApprove ? "available" : routeTabs.selectedId;
  return (
    <Page>
      <PageHeader
        title="Versions and updates"
        description="Updates published for this app, and records of each update: its approval, the backup taken before it, and acceptance for clinical use."
      />
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
    </Page>
  );
}
