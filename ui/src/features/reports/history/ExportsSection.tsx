import { useMemo } from "react";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import type { ExportView, RevisionSummary } from "../contract";
import { AVAILABILITY, EXPORT_PURPOSES } from "../labels";

interface ExportsSectionProps {
  exports: readonly ExportView[];
  /** The report's revisions, to say whether an exported one is current. */
  revisions: readonly RevisionSummary[];
}

/**
 * Every PDF export of the report's revisions and notices, most recent first, each recorded
 * and none of them a delivery (NFR-07, REQ-RPT-011).
 */
export function ExportsSection({ exports, revisions }: ExportsSectionProps) {
  const columns = useMemo(() => exportColumns(revisions), [revisions]);
  return (
    <Section title="PDF exports">
      {exports.length === 0 ? (
        <p className={styles.hint}>Not exported.</p>
      ) : (
        <>
          <p className={styles.hint}>An export is not a delivery.</p>
          <DataGrid
            label="PDF exports"
            columns={columns}
            rows={exports}
            getRowId={(row) => row.exportId}
          />
        </>
      )}
    </Section>
  );
}

function exportColumns(revisions: readonly RevisionSummary[]): DataGridColumn<ExportView>[] {
  const exported = (row: ExportView) => {
    if (row.withdrawalNotice) return "Withdrawal notice";
    const revision = revisions.find((each) => each.revisionNo === row.revisionNo);
    const availability = revision && AVAILABILITY[revision.availability].label.toLowerCase();
    return `Revision ${row.revisionNo}${availability ? ` (${availability})` : ""}`;
  };
  return [
    {
      id: "exported",
      header: "Exported",
      cell: (row) => <Timestamp value={row.exportedAt} />,
      sortValue: (row) => row.exportedAt,
      defaultSort: "desc",
      width: 190,
    },
    { id: "revision", header: "Revision", cell: exported, width: 190 },
    {
      id: "purpose",
      header: "Purpose",
      cell: (row) => EXPORT_PURPOSES.find((purpose) => purpose.id === row.purpose)?.label,
    },
    { id: "by", header: "By", cell: (row) => row.exportedBy.displayName, width: 160 },
    { id: "file", header: "File", cell: (row) => row.fileName },
  ];
}
