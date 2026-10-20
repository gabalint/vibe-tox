import { useMemo, useState } from "react";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { EmptyState } from "@/components/EmptyState";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import type { HostHealthView } from "@/generated/HostHealthView";
import { allowedActionItems, allowedRowActions } from "@/lib/allowedActions";
import { useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { ATTEST_HEALTH_CHECK, HEALTH_QUERIES, MANAGE_CONFIGURATION } from "../contract";
import { CheckPanel } from "./CheckPanel";
import { CHECK_COLUMNS } from "./checkColumns";
import { severityRank } from "./healthLabels";
import { NoteDialog } from "./NoteDialog";

const checkId = (check: HealthCheckView) => check.check;
const ACTIONS = [{ id: ATTEST_HEALTH_CHECK, label: "Attest" }];

/**
 * The checks with the overall state (REQ-OPS-008), most urgent first, so an alert is never
 * below the fold while the top bar reports it. A check opens in a side panel (`?open=` takes
 * its code). Checks the engine marks attestable, such as a UPS it cannot read, can be attested
 * by people with `manage_configuration`, from the panel or the row's menu.
 */
export function ChecksSection({ health }: { health: HostHealthView }) {
  const canAttest = useHasCapability(MANAGE_CONFIGURATION);
  const record = useOpenRecord();
  const [attesting, setAttesting] = useState<HealthCheckView | null>(null);
  // Stable sort: checks of the same state keep the engine's order.
  const checks = useMemo(
    () => health.checks.toSorted((a, b) => severityRank(a.state) - severityRank(b.state)),
    [health.checks],
  );
  // Engine request: `allowedActions` on `HealthCheckView` would replace this condition.
  const allowed = (check: HealthCheckView) =>
    canAttest && check.attestable ? [ATTEST_HEALTH_CHECK] : [];
  const actionsOf = (check: HealthCheckView) =>
    allowedActionItems(ACTIONS, allowed(check), () => setAttesting(check));
  const rowActions = allowedRowActions(ACTIONS, {
    label: (row: HealthCheckView) => row.label,
    allowed,
    onAction: (_, row) => setAttesting(row),
  });
  const panel = openRecordPanel(
    record,
    checks,
    checkId,
    (check) => (
      <CheckPanel
        check={check}
        actions={actionsOf(check)}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
    <p>The app reports no such health check.</p>,
  );
  return (
    <>
      <DetailLayout panel={panel}>
        {checks.length === 0 ? (
          <EmptyState
            title="No checks reported"
            description="The app has not reported any check."
          />
        ) : (
          <DataGrid
            label="Health checks"
            columns={CHECK_COLUMNS}
            rows={checks}
            getRowId={checkId}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            onRowAction={(row, order) => record.open(row.check, order)}
            rowActions={rowActions}
            fill
          />
        )}
      </DetailLayout>
      {attesting && (
        <NoteDialog
          title="Attest check"
          submitLabel="Attest"
          command={{ name: ATTEST_HEALTH_CHECK, invalidates: HEALTH_QUERIES }}
          toInput={(note) => ({ check: attesting.check, note })}
          successTitle="Attestation recorded"
          onClose={() => setAttesting(null)}
        >
          <p>
            {`${attesting.label}: you confirm that you checked this by hand. The app records who attested it and until when the attestation is valid.`}
          </p>
        </NoteDialog>
      )}
    </>
  );
}
