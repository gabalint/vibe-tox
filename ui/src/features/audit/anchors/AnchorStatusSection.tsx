import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import { useToast } from "@/components/Toast";
import type { AnchorStatusView } from "@/generated/AnchorStatusView";
import type { ExportReport } from "@/generated/ExportReport";
import type { StreamAnchorView } from "@/generated/StreamAnchorView";
import { useCommandMutation, useCommandQuery, useHasCapability } from "@/lib/commands";
import { streamText } from "./anchorText";
import { ANCHOR_QUERIES, BACKUP_RESTORE, EXPORT_ANCHORS_NOW, GET_ANCHOR_STATUS } from "./contract";

const STREAM_COLUMNS: readonly DataGridColumn<StreamAnchorView>[] = [
  { id: "stream", header: "Stream", cell: (row) => streamText(row.stream) },
  { id: "head", header: "Newest event", cell: (row) => row.headSeq, align: "end" },
  {
    id: "sealed",
    header: "Sealed through event",
    cell: (row) => row.sealedThroughSeq,
    align: "end",
  },
  {
    id: "anchored",
    header: "Anchored through event",
    cell: (row) => row.anchoredThroughSeq,
    align: "end",
  },
  {
    id: "oldest",
    header: "Oldest event not anchored off this computer",
    cell: (row) => (row.oldestUnanchoredAt ? <Timestamp value={row.oldestUnanchoredAt} /> : "None"),
  },
];

/**
 * What one export wrote. Targets that failed or were not reachable keep their seals pending,
 * so the toast warns about them instead of reporting success.
 */
function exportToast(report: ExportReport) {
  const written = `Seals written: ${report.sealsWritten}. Targets written: ${report.targetsWritten}.`;
  const missed = report.targetsFailed + report.targetsUnavailable;
  if (missed === 0) {
    return { tone: "success" as const, title: "Anchors exported", description: written };
  }
  return {
    tone: "warning" as const,
    title: "Anchors not written to every target",
    description: `${written} Targets failed: ${report.targetsFailed}. Targets not reachable: ${report.targetsUnavailable}. Their anchors stay pending.`,
  };
}

/**
 * How far each audit stream is sealed and anchored, the lag as the engine reports it, and the
 * off-host targets that hold anchors. Lag never blocks release (ADR 0008).
 */
export function AnchorStatusSection() {
  const toast = useToast();
  const canExport = useHasCapability(BACKUP_RESTORE);
  const status = useCommandQuery<AnchorStatusView>(GET_ANCHOR_STATUS);
  const exportNow = useCommandMutation<ExportReport>(EXPORT_ANCHORS_NOW, {
    invalidates: ANCHOR_QUERIES,
  });
  const onExport = () =>
    exportNow.mutate({}, { onSuccess: (report) => toast.show(exportToast(report)) });
  const action = canExport && (
    <Button isPending={exportNow.isPending} onPress={onExport}>
      Export anchors now
    </Button>
  );
  return (
    <Section title="Status" actions={action}>
      {exportNow.error && <ErrorState title="Anchors not exported" error={exportNow.error} />}
      <QueryContent query={status} what="anchor status">
        {(data) => <StatusContent status={data} />}
      </QueryContent>
    </Section>
  );
}

function StatusContent({ status }: { status: AnchorStatusView }) {
  const { lastExport } = status;
  return (
    <>
      {status.offHostTargets === 0 && (
        <Alert tone="warning" title="No off-host target">
          Anchors are kept only on this computer's own disks, where they give little protection
          against changes made on this computer. Add a removable drive or network share as a backup
          target.
        </Alert>
      )}
      <Facts
        layout="compact"
        items={[
          {
            label: "Anchor lag",
            value: status.lagMinutes === null ? "Not reported" : `${status.lagMinutes} minutes`,
          },
          { label: "Off-host targets", value: String(status.offHostTargets) },
          {
            label: "Last export",
            value: lastExport ? (
              <>
                <Timestamp value={lastExport.at} /> to {lastExport.targetLabel}
              </>
            ) : (
              "None yet"
            ),
          },
        ]}
      />
      <p>Anchor lag never blocks the release of reports.</p>
      <DataGrid
        label="Audit streams"
        columns={STREAM_COLUMNS}
        rows={status.streams}
        getRowId={(row) => row.stream}
      />
    </>
  );
}
