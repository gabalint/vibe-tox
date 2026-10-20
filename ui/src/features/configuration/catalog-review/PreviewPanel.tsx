import type { DataGridColumn } from "@/components/DataGrid";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { type ActionItem, RecordActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { CatalogPreviewSummary } from "@/generated/CatalogPreviewSummary";
import type { OpenRecord } from "@/lib/openRecord";
import { usePersonText } from "@/lib/person";
import layout from "@/styles/layout.module.css";

/** Unreviewed reports wait for a person; a reviewed one is finished, so plain. */
function ReviewState({ preview }: { preview: CatalogPreviewSummary }) {
  const person = usePersonText();
  if (preview.reviewedAt === null) return <StatusBadge tone="warning">Not reviewed</StatusBadge>;
  return (
    <>
      Reviewed <Timestamp value={preview.reviewedAt} /> by{" "}
      {person(preview.reviewedBy ?? "", preview.reviewedByDisplay)}
    </>
  );
}

function Generated({ preview }: { preview: CatalogPreviewSummary }) {
  const person = usePersonText();
  return (
    <>
      <Timestamp value={preview.generatedAt} /> by{" "}
      {person(preview.generatedBy, preview.generatedByDisplay)}
    </>
  );
}

export const PREVIEW_COLUMNS: readonly DataGridColumn<CatalogPreviewSummary>[] = [
  { id: "id", header: "Report", identifier: true, cell: (row) => row.id, width: 320 },
  {
    id: "generated",
    header: "Generated",
    cell: (row) => <Generated preview={row} />,
    sortValue: (row) => row.generatedAt,
  },
  { id: "drafts", header: "Drafts", align: "end", cell: (row) => row.drafts, width: 90 },
  { id: "review", header: "Review", wrap: true, cell: (row) => <ReviewState preview={row} /> },
];

interface PreviewPanelProps {
  preview: CatalogPreviewSummary;
  actions: readonly ActionItem[];
  record: OpenRecord;
}

/** One Catalog Preview report: what identifies the printed copy, and its review. */
export function PreviewPanel({ preview, actions, record }: PreviewPanelProps) {
  return (
    <DetailPanel
      identifier={preview.id}
      status={preview.reviewedAt === null ? <ReviewState preview={preview} /> : undefined}
      actions={<RecordActions items={actions} />}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    >
      <Facts
        items={[
          { label: "Generated", value: <Generated preview={preview} /> },
          {
            label: "Content digest",
            value: <span className={layout.code}>{preview.contentDigest}</span>,
          },
          { label: "Drafts listed", value: String(preview.drafts) },
          {
            label: "Blank cutoffs to confirm",
            value: preview.reviewedAt === null ? String(preview.blankCutoffs.length) : "None",
          },
          { label: "Review", value: <ReviewState preview={preview} /> },
          {
            label: "External approval reference",
            value: preview.externalApprovalReference ?? "Not recorded",
          },
        ]}
      />
    </DetailPanel>
  );
}
