import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { PreviewReportLayoutInput } from "@/generated/PreviewReportLayoutInput";
import type { ReportLayout } from "@/generated/ReportLayout";
import type { ReportLayoutsView } from "@/generated/ReportLayoutsView";
import { CommandRefusal } from "@/lib/CommandRefusal";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { ActiveLayout, DraftLayout } from "./CurrentLayouts";
import { GET_REPORT_LAYOUTS, PREVIEW_REPORT_LAYOUT } from "./contract";
import { LayoutCards } from "./LayoutCards";
import { LayoutVersions } from "./LayoutVersions";

/** "Active: Standard", with the draft waiting for approval when there is one. */
function LayoutStatus({ view }: { view: ReportLayoutsView }) {
  if (view.draft) {
    return (
      <StatusBadge tone="warning">
        {`Active: ${view.inForce.name}, ${view.draft.name} waiting for approval`}
      </StatusBadge>
    );
  }
  return <StatusBadge tone="neutral">{`Active: ${view.inForce.name}`}</StatusBadge>;
}

/**
 * Report templates (REQ-RPT-007): one laboratory-wide report layout. An administrator saves a
 * ready-made layout as a draft; a quality manager previews it and approves or rejects it with a
 * reason. Only reports prepared after the approval use the new layout; released reports and
 * their PDFs never change. Previews open in a report window on built-in synthetic data, marked
 * "PREVIEW – NOT A REPORT".
 */
export function ReportTemplatesPage() {
  const layouts = useCommandQuery<ReportLayoutsView>(GET_REPORT_LAYOUTS);
  const preview = useCommandMutation<null, { input: PreviewReportLayoutInput }>(
    PREVIEW_REPORT_LAYOUT,
    { invalidates: [] },
  );
  const show = (layout: ReportLayout) => preview.mutate({ input: { layout } });
  return (
    <Page>
      <PageHeader
        title="Report templates"
        description="The layout of released reports. Only the layout changes; every layout prints the same content."
        status={layouts.data && <LayoutStatus view={layouts.data} />}
      />
      {preview.error && (
        <CommandRefusal title="Preview not shown" error={preview.error} lead={null} />
      )}
      <QueryContent query={layouts} what="report layouts" loadingRows={4}>
        {(view) => (
          <div className={styles.stack}>
            <ActiveLayout inForce={view.inForce} onPreview={show} />
            {view.draft && (
              <DraftLayout
                draft={view.draft}
                allowedActions={view.allowedActions}
                onPreview={show}
              />
            )}
            <Section title="Layouts">
              <LayoutCards view={view} onPreview={show} />
            </Section>
            {view.versions.length > 0 && (
              <Section title="History">
                <LayoutVersions versions={view.versions} />
              </Section>
            )}
          </div>
        )}
      </QueryContent>
    </Page>
  );
}
