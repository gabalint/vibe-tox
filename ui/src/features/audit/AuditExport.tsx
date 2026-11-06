import { ChevronDown } from "lucide-react";
import { useContext } from "react";
import { MenuTrigger } from "react-aria-components";
import { Button } from "@/components/Button";
import { ActionMenu } from "@/components/RowActions";
import { useToast } from "@/components/Toast";
import type { AuditExportFormat } from "@/generated/AuditExportFormat";
import type { ExportAuditEventsInput } from "@/generated/ExportAuditEventsInput";
import type { ExportedAuditFile } from "@/generated/ExportedAuditFile";
import { useCommandMutation, useHasCapability } from "@/lib/commands";
import { LabTimeZoneContext } from "@/lib/format";
import { auditInput } from "./AuditEvents";
import type { AuditFilter } from "./contract";

/** The page size the filter carries; the engine reads every page of an export itself. */
const EXPORT_PAGE = 500;

/**
 * Export of every event the filters match, as CSV or PDF into the Downloads folder
 * (REQ-OPS-001), for people with `export` besides `view_audit`. The engine records the export
 * with its filters; patient values appear only where the exporter's roles may read them.
 */
export function useAuditExport(filter: AuditFilter) {
  const canExport = useHasCapability("export");
  const timeZone = useContext(LabTimeZoneContext);
  const toast = useToast();
  const mutation = useCommandMutation<ExportedAuditFile, { input: ExportAuditEventsInput }>(
    "export_audit_events",
    { invalidates: [] },
  );
  const run = (format: AuditExportFormat) =>
    mutation.mutate(
      { input: { format, filter: { ...auditInput(filter, timeZone), limit: EXPORT_PAGE } } },
      {
        onSuccess: (file) =>
          toast.show({
            tone: "success",
            title: `Saved ${file.fileName} to your Downloads folder.`,
            description: `${file.events} ${file.events === 1 ? "event" : "events"} exported.`,
          }),
      },
    );
  return { canExport, run, mutation };
}

/** The "Export" menu of the page header: CSV for analysis, PDF for printing. */
export function AuditExportMenu({ exporter }: { exporter: ReturnType<typeof useAuditExport> }) {
  if (!exporter.canExport) return null;
  return (
    <MenuTrigger>
      <Button isPending={exporter.mutation.isPending}>
        Export
        <ChevronDown aria-hidden="true" />
      </Button>
      <ActionMenu
        items={[
          { id: "csv", label: "CSV, for analysis", onAction: () => exporter.run("csv") },
          { id: "pdf", label: "PDF, for printing", onAction: () => exporter.run("pdf") },
        ]}
      />
    </MenuTrigger>
  );
}
