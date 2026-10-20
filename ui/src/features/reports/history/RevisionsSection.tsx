import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import { useOpenRecordPage } from "@/lib/recordSequence";
import type { RevisionSummary } from "../contract";
import { RELATIONSHIP } from "../labels";
import { revisionPath } from "../paths";
import { statusColumns } from "./columns";

const COLUMNS: readonly DataGridColumn<RevisionSummary>[] = [
  {
    id: "revision",
    header: "Revision",
    cell: (row) => `Revision ${row.revisionNo}`,
    sortValue: (row) => row.revisionNo,
    identifier: true,
    width: 110,
  },
  { id: "relationship", header: "Kind", cell: (row) => RELATIONSHIP[row.relationship], width: 140 },
  ...statusColumns<RevisionSummary>(),
  {
    id: "released",
    header: "Released",
    cell: (row) => (
      <>
        <Timestamp value={row.releasedAt} /> by {row.releasedBy.displayName}
      </>
    ),
  },
  { id: "reason", header: "Reason for correction", cell: (row) => row.amendmentReason ?? "—" },
];

interface RevisionsSectionProps {
  revisions: readonly RevisionSummary[];
  /** The report's page title, the breadcrumb back from a revision. */
  reportTitle: string;
}

/**
 * Every released revision, earliest first (REQ-RPT-002). Earlier revisions stay readable
 * exactly as issued; a correction names its reason. A revision opens on its page, with
 * Previous and Next through the report's revisions.
 */
export function RevisionsSection({ revisions, reportTitle }: RevisionsSectionProps) {
  const openRevision = useOpenRecordPage(revisionPath, reportTitle);
  return (
    <Section title="Revisions">
      <DataGrid
        label="Revisions"
        columns={COLUMNS}
        rows={revisions}
        getRowId={(row) => row.revisionId}
        onRowAction={(row, order) => openRevision(row.revisionId, order)}
      />
    </Section>
  );
}
