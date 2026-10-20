import { Facts } from "@/components/Facts";
import type { ToxReportData } from "@/generated/ToxReportData";
import layout from "@/styles/layout.module.css";
import { COMPLETENESS, RELATIONSHIP } from "../labels";
import styles from "../reports.module.css";
import { ResultGroups } from "./ResultGroups";

const UNKNOWN = "Unknown";
const NONE = "None";

/**
 * Exactly what the report prints (REQ-RPT-001, REQ-RPT-008): every string comes from the
 * engine's `ToxReportData`, the same data the PDF is rendered from. The PDF itself is reached
 * through export, because not every platform's web view can display PDFs.
 */
export function ReportContent({ content }: { content: ToxReportData }) {
  const { report, laboratory, validity } = content;
  return (
    <article aria-label={`Printed content of report ${report.number}`} className={styles.document}>
      <h3 className={layout.subheading}>Report</h3>
      <Facts
        items={[
          { label: "Report number", value: report.number },
          { label: "Revision", value: report.revision },
          { label: "Status", value: COMPLETENESS[report.completeness].label },
          { label: "Relationship", value: RELATIONSHIP[report.relationship] },
          ...(report.supersedes
            ? [
                {
                  label: "Follows",
                  value: `Revision ${report.supersedes.revision}, released ${report.supersedes.releasedAt}`,
                },
              ]
            : []),
          ...(report.amendmentReason
            ? [{ label: "Reason for correction", value: report.amendmentReason }]
            : []),
          {
            label: report.generatedAt ? "Generated" : "Issued",
            value: report.generatedAt ?? report.issuedAt,
          },
        ]}
      />
      {(report.changeSummary?.length ?? 0) > 0 && (
        <section aria-label="Changes in this correction">
          <h3 className={layout.subheading}>Changes in this correction</h3>
          <ul className={layout.list}>
            {(report.changeSummary ?? []).map((change) => (
              <li key={change}>{change}</li>
            ))}
          </ul>
        </section>
      )}
      <h3 className={layout.subheading}>
        {report.generatedAt ? "Reporting laboratory contact" : "Performing laboratory"}
      </h3>
      <Facts
        items={[
          { label: "Laboratory", value: laboratory.name },
          { label: "Address", value: laboratory.addressLines.join(", ") },
          { label: "CLIA number", value: laboratory.cliaId },
          { label: "Director", value: laboratory.director },
          { label: "Phone", value: laboratory.phone },
        ]}
      />
      <PatientAndSpecimen content={content} />
      <ResultGroups content={content} />
      <h3 className={layout.subheading}>Specimen validity</h3>
      <Facts
        items={[
          { label: "Outcome", value: validity.outcome },
          { label: "Notes", value: listText(validity.notes) },
          {
            label: report.generatedAt ? "Prepared for release by" : "Released by",
            value: content.releasedBy.name,
          },
        ]}
      />
    </article>
  );
}

function listText(values: readonly string[]): string {
  return values.length === 0 ? NONE : values.join("; ");
}

/** Who the report is about, who ordered it, and the specimen, as they will print. */
function PatientAndSpecimen({ content }: { content: ToxReportData }) {
  const { subject, order, specimen } = content;
  return (
    <>
      <h3 className={layout.subheading}>Patient and order</h3>
      <Facts
        items={[
          { label: "Patient", value: subject.name },
          { label: "Birth date", value: subject.birthDate },
          { label: "Sex", value: subject.sex ?? UNKNOWN },
          {
            label: "Identifiers",
            value: listText(
              subject.identifiers.map((each) => `${each.type} ${each.value} (${each.authority})`),
            ),
          },
          {
            label: "Ordering practitioner",
            value: `${order.practitioner.name}, NPI ${order.practitioner.npi}`,
          },
          { label: "Client", value: order.client },
          { label: "Location", value: order.location ?? NONE },
          { label: "Placer order", value: order.placerOrder ?? NONE },
        ]}
      />
      <h3 className={layout.subheading}>Specimen</h3>
      <Facts
        items={[
          { label: "Accession", value: specimen.accession },
          { label: "Specimen type", value: specimen.matrix },
          { label: "Containers", value: listText(specimen.containerLabels) },
          { label: "Collected", value: specimen.collected ?? UNKNOWN },
          { label: "Received", value: specimen.received ?? UNKNOWN },
          { label: "Condition", value: listText(specimen.conditionNotes) },
          {
            label: "Collection observations",
            value: listText(
              specimen.collectionObservations.map(
                (each) => `${each.code}: ${each.value} (${each.observedBy})`,
              ),
            ),
          },
          {
            label: "Medications",
            value: listText(
              content.medications.map(
                (each) => `${each.name} (${each.source}, as of ${each.asOf ?? UNKNOWN})`,
              ),
            ),
          },
        ]}
      />
    </>
  );
}
