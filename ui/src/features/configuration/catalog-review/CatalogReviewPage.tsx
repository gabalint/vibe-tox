import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import type { ActionItem } from "@/components/RowActions";
import { useToast } from "@/components/Toast";
import type { CatalogPreviewPdfInput } from "@/generated/CatalogPreviewPdfInput";
import type { CatalogPreviewSummary } from "@/generated/CatalogPreviewSummary";
import type { CatalogReviewApproved } from "@/generated/CatalogReviewApproved";
import type { ExportedReportFile } from "@/generated/ExportedReportFile";
import { CommandRefusal } from "@/lib/CommandRefusal";
import { useCommandMutation, useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { APPROVE_CONFIGURATION } from "../contract";
import {
  EXPORT_CATALOG_PREVIEW_PDF,
  GENERATE_CATALOG_PREVIEW,
  LIST_CATALOG_PREVIEWS,
  VIEW_CATALOG_PREVIEW_PDF,
} from "./contract";
import { PreviewList } from "./PreviewList";
import { ReviewDialog } from "./ReviewDialog";
import { previewRefusal, viewRefusal } from "./refusals";

/** REQ-OPS-015: laboratory catalog review, available without any starter-data load. */
export function CatalogReviewPage() {
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  const previews = useCommandQuery<CatalogPreviewSummary[]>(LIST_CATALOG_PREVIEWS);
  const record = useOpenRecord();
  const toast = useToast();
  const [reviewing, setReviewing] = useState<string | null>(null);
  const [approved, setApproved] = useState<CatalogReviewApproved | null>(null);
  const generate = useCommandMutation<string>(GENERATE_CATALOG_PREVIEW, {
    invalidates: [LIST_CATALOG_PREVIEWS],
  });
  const view = useCommandMutation<null, { input: CatalogPreviewPdfInput }>(
    VIEW_CATALOG_PREVIEW_PDF,
    { invalidates: [] },
  );
  const exportPdf = useCommandMutation<ExportedReportFile, { input: CatalogPreviewPdfInput }>(
    EXPORT_CATALOG_PREVIEW_PDF,
    { invalidates: [] },
  );
  const show = (previewId: string, order?: readonly string[]) => {
    record.open(previewId, order);
    view.mutate({ input: { previewId } });
  };
  const runGenerate = () =>
    generate.mutate(
      {},
      {
        onSuccess: (id) => {
          toast.show({ tone: "success", title: "Catalog Preview generated" });
          show(id);
        },
      },
    );
  const savePdf = (previewId: string) =>
    exportPdf.mutate(
      { input: { previewId } },
      {
        onSuccess: (file) =>
          toast.show({
            tone: "success",
            title: `Saved ${file.fileName} to your Downloads folder.`,
          }),
      },
    );
  const actionsOf = (preview: CatalogPreviewSummary): ActionItem[] => [
    { id: "view", label: "View report", onAction: () => show(preview.id) },
    { id: "export", label: "Save PDF", onAction: () => savePdf(preview.id) },
    ...(canApprove && preview.reviewedAt === null
      ? [
          {
            id: "review",
            label: "Approve reviewed catalog",
            onAction: () => setReviewing(preview.id),
          },
        ]
      : []),
  ];
  const target = previews.data?.find((row) => row.id === reviewing);
  return (
    <Page layout="collection">
      <PageHeader
        title="Catalog review"
        description="Review catalog values, regardless of how they were entered."
        actions={
          <Button variant="primary" isPending={generate.isPending} onPress={runGenerate}>
            Generate Catalog Preview
          </Button>
        }
      />
      {generate.error && (
        <CommandRefusal
          title="Not generated"
          error={generate.error}
          lead={previewRefusal(generate.error)}
        />
      )}
      {view.error && (
        <CommandRefusal title="Not shown" error={view.error} lead={viewRefusal(view.error)} />
      )}
      {exportPdf.error && (
        <CommandRefusal
          title="Not saved"
          error={exportPdf.error}
          lead={previewRefusal(exportPdf.error)}
        />
      )}
      {approved && <ApprovedAlert approved={approved} />}
      <PreviewList previews={previews} record={record} actionsOf={actionsOf} onOpen={show} />
      {target && (
        <ReviewDialog
          preview={target}
          onApproved={setApproved}
          onClose={() => setReviewing(null)}
        />
      )}
    </Page>
  );
}

function ApprovedAlert({ approved }: { approved: CatalogReviewApproved }) {
  return (
    <Alert tone="success" title="Reviewed catalog approved">
      <Facts
        items={[
          { label: "Methods approved", value: String(approved.methods) },
          { label: "Tests and panels approved", value: String(approved.orderables) },
          { label: "Validity policies approved", value: String(approved.validityPolicies) },
          {
            label: "Blank cutoffs confirmed not applicable",
            value: String(approved.confirmedBlankCutoffs),
          },
        ]}
      />
    </Alert>
  );
}
