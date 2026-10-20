import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount } from "@/components/FindField";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Toolbar } from "@/components/Toolbar";
import type { InstalledVersionView } from "@/generated/InstalledVersionView";
import { allowedActionItems, allowedRowActions } from "@/lib/allowedActions";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import {
  ACCEPT_INSTALLED_VERSION,
  APPROVE_CONFIGURATION,
  LIST_INSTALLED_VERSIONS,
} from "../contract";
import { AcceptVersionDialog } from "./AcceptVersionDialog";
import { VersionPanel } from "./VersionPanel";
import { VERSION_COLUMNS } from "./versionColumns";

const versionId = (row: InstalledVersionView) => row.version;
const ACTIONS = [{ id: ACCEPT_INSTALLED_VERSION, label: "Accept this version for clinical use" }];

/**
 * The versions that have run, newest first (REQ-OPS-006, NFR-24). A version opens in a side
 * panel (`?open=` takes the version); someone with `approve_configuration` accepts the running
 * version for clinical use from the panel or the row's menu.
 */
export function InstalledVersions() {
  const query = useCommandQuery<InstalledVersionView[]>(LIST_INSTALLED_VERSIONS);
  const record = useOpenRecord();
  const canAccept = useHasCapability(APPROVE_CONFIGURATION);
  const [accepting, setAccepting] = useState<InstalledVersionView | null>(null);
  // Engine request: `allowedActions` on `InstalledVersionView` would replace this condition.
  const allowed = (row: InstalledVersionView) =>
    canAccept && row.status === "awaiting_acceptance" && row.running
      ? [ACCEPT_INSTALLED_VERSION]
      : [];
  const actionsOf = (row: InstalledVersionView) =>
    allowedActionItems(ACTIONS, allowed(row), () => setAccepting(row));
  const running = query.data?.find((row) => row.running);
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: InstalledVersionView) => `version ${row.version}`,
    allowed,
    onAction: (_, row) => setAccepting(row),
  });
  const panel = openRecordPanel(
    record,
    query.data,
    versionId,
    (version) => (
      <VersionPanel
        version={version}
        actions={actionsOf(version)}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <p>No such version has run.</p>,
  );
  return (
    <>
      {running && (
        <Section
          title={`Running version ${running.version}`}
          actions={
            allowed(running).length > 0 && (
              <Button variant="primary" onPress={() => setAccepting(running)}>
                Accept running version
              </Button>
            )
          }
        >
          <p>
            This version is detected automatically. Acceptance records the laboratory’s checks for
            the app already installed; it needs a note and identity confirmation.
          </p>
        </Section>
      )}
      <DetailLayout panel={panel}>
        <Toolbar
          variant="collection"
          count={
            query.isSuccess && (
              <FindCount
                shown={query.data.length}
                total={query.data.length}
                noun="installed versions"
                one="installed version"
              />
            )
          }
        />
        <QueryContent
          query={query}
          what="installed versions"
          loadingRows={4}
          empty={{
            when: (rows) => rows.length === 0,
            title: "No versions recorded",
            description: "Each version of the app is recorded the first time it starts.",
          }}
        >
          {(rows) => (
            <DataGrid
              label="Installed versions"
              columns={VERSION_COLUMNS}
              rows={rows}
              getRowId={versionId}
              openRowId={record.openId}
              onOrderChange={record.followOrder}
              onRowAction={(row, order) => record.open(row.version, order)}
              rowActions={rowActions}
              fill
            />
          )}
        </QueryContent>
      </DetailLayout>
      {accepting && (
        <AcceptVersionDialog version={accepting.version} onClose={() => setAccepting(null)} />
      )}
    </>
  );
}
