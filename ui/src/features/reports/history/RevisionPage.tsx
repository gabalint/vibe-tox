import { useState } from "react";
import { Link, useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { useCommandQuery } from "@/lib/commands";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { ReportContent } from "../content/ReportContent";
import {
  GET_REPORT,
  GET_REPORT_REVISION,
  type ReportDetail,
  type RevisionDetail,
} from "../contract";
import { ExportDialog } from "../delivery/ExportDialog";
import { AVAILABILITY, COMPLETENESS, revisionText } from "../labels";
import { ParentIdentity } from "../ParentIdentity";
import { reportPath, revisionPath } from "../paths";

/** Where the breadcrumb leads before the revision, and so its report, is known. */
const HOME = { label: "Reports", href: "/reports" };

/**
 * One released revision exactly as issued (REQ-RPT-001, NFR-07): its printed content, how it
 * was produced, and its digests. Earlier revisions stay readable after any catalog or template
 * change. The patient comes from the revision's report, read once the revision names it,
 * since the revision itself carries no identity. Opening this screen, and so the report, is
 * recorded in the audit trail.
 */
export function RevisionPage() {
  const { revisionId = "" } = useParams();
  const revision = useCommandQuery<RevisionDetail>(GET_REPORT_REVISION, {
    input: { revisionId },
  });
  const data = revision.data;
  const availability = data && AVAILABILITY[data.summary.availability];
  // Opened from elsewhere, the breadcrumb leads to the revision's report.
  const home = data
    ? { label: `Report ${data.reportNumber}`, href: reportPath(data.reportId) }
    : HOME;
  const { list, sequence } = useRecordSequence(revisionId, home, revisionPath);
  const revisionLabel = data ? `Revision ${data.summary.revisionNo}` : "Report revision";
  // One header in every state, so Previous and Next keep focus while the next one loads.
  return (
    <Page>
      <PageHeader
        title={
          data
            ? `Report ${data.reportNumber}, revision ${data.summary.revisionNo}`
            : "Report revision"
        }
        description="Opening this screen is recorded in the audit trail."
        status={
          availability && <StatusBadge tone={availability.tone}>{availability.label}</StatusBadge>
        }
        breadcrumbs={[list, { label: revisionLabel, href: revisionPath(revisionId) }]}
        sequence={sequence}
        actions={data && <ExportAction detail={data} />}
      />
      {data && <ReportIdentity reportId={data.reportId} />}
      <RefreshControl query={revision} />
      <QueryContent query={revision} what="report revision">
        {(loaded) => <Revision detail={loaded} />}
      </QueryContent>
    </Page>
  );
}

/**
 * The patient and specimen of the revision's report (`get_report`, an audited read of the
 * report this page already names). Engine request: `subject` and `accessionNumber` on
 * `RevisionDetail`, so this second read can go.
 */
function ReportIdentity({ reportId }: { reportId: string }) {
  const report = useCommandQuery<ReportDetail>(GET_REPORT, { input: { reportId } });
  return <ParentIdentity parent={report} />;
}

function ExportAction({ detail }: { detail: RevisionDetail }) {
  const [isExporting, setExporting] = useState(false);
  return (
    <>
      <Button onPress={() => setExporting(true)}>Export PDF</Button>
      {isExporting && (
        <ExportDialog
          revisionId={detail.summary.revisionId}
          reportNumber={detail.reportNumber}
          onClose={() => setExporting(false)}
        />
      )}
    </>
  );
}

function Revision({ detail }: { detail: RevisionDetail }) {
  const { summary, content, provenance } = detail;
  const digest = (value: string) => <span className={styles.code}>{value}</span>;
  return (
    <div className={styles.stack}>
      <Section title="Revision">
        <Facts
          items={[
            {
              label: "Report",
              value: <Link to={reportPath(detail.reportId)}>{detail.reportNumber}</Link>,
            },
            {
              label: "Revision",
              value: `${revisionText(summary.revisionNo, summary.relationship)}, ${COMPLETENESS[summary.completeness].label.toLowerCase()}`,
            },
            {
              label: "Released",
              value: (
                <>
                  <Timestamp value={summary.releasedAt} /> by {summary.releasedBy.displayName}
                </>
              ),
            },
            {
              label: "Template",
              value: `${provenance.templateId} version ${provenance.templateVersion}, renderer ${provenance.rendererVersion}`,
            },
            { label: "Fonts", value: provenance.fonts.join(", ") },
            { label: "Content digest", value: digest(provenance.contentSha256) },
            { label: "Data digest", value: digest(provenance.dataSha256) },
            { label: "File digest", value: digest(provenance.artifactSha256) },
            { label: "Template digest", value: digest(provenance.templateSha256) },
          ]}
        />
      </Section>
      <Section title="Content as issued">
        <ReportContent content={content} />
      </Section>
    </div>
  );
}
