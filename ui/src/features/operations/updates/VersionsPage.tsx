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
import { InstalledVersions } from "./InstalledVersions";
import { RecordApprovalDialog } from "./RecordApprovalDialog";
import { RequestUpdateBackup } from "./RequestUpdateBackup";

const PATH = "/operations/versions";

const TABS = [
  { id: "installed", label: "Installed versions" },
  { id: "approvals", label: "Update approvals" },
] as const;

/**
 * Installed versions and their update records (REQ-OPS-006, NFR-24; design brief §7): the
 * approval before an update, the verified pre-update backup, and acceptance for clinical use
 * of the version that now runs. The app installs no update itself. The approvals are a second
 * tab for people with `manage_configuration`; the base path shows the installed versions.
 */
export function VersionsPage() {
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const canApprove = session.data?.capabilities.includes(MANAGE_CONFIGURATION) ?? false;
  const [approving, setApproving] = useState(false);
  const { selectedId: tab, tabs } = useRouteTabs(PATH, TABS);
  return (
    <Page>
      <PageHeader
        title="Versions and updates"
        actions={
          canApprove && <Button onPress={() => setApproving(true)}>Record update approval</Button>
        }
        description="Records of each update: its approval, the backup taken before it, and acceptance for clinical use."
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
        {() =>
          canApprove ? (
            <Tabs
              contentLayout="collection"
              label="Versions and updates"
              tabs={tabs}
              selectedId={tab}
            >
              {tab === "installed" ? <InstalledVersions /> : <ApprovalsSection />}
            </Tabs>
          ) : (
            <InstalledVersions />
          )
        }
      </QueryContent>
      {approving && <RecordApprovalDialog onClose={() => setApproving(false)} />}
    </Page>
  );
}
