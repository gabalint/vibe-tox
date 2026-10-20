import type { ReactNode } from "react";
import { Section } from "@/components/Section";
import type { AuthorityEvidenceView } from "@/generated/AuthorityEvidenceView";
import type { OrderDetail } from "@/generated/OrderDetail";
import { labelOf } from "@/lib/choices";
import styles from "../accessioning.module.css";
import { orNotRecorded } from "../subjects";
import { AUTHORITY_KINDS, MEDICATION_SOURCES } from "./labels";

type Evidence = OrderDetail["evidence"];

/** Evidence rows are append-only: an amendment adds a version and retires rows. */
type Versioned = Pick<AuthorityEvidenceView, "evidenceId" | "addedInVersion" | "removedInVersion">;

/**
 * Evidence as supplied, version by version (REQ-ORD-005, REQ-ORD-007). Removed entries stay
 * visible with the version that removed them. Point-of-care results are context only.
 */
export function OrderEvidence({ evidence }: { evidence: Evidence }) {
  return (
    <Section title="Evidence as supplied">
      <EvidenceList
        title="Authority to test"
        rows={evidence.authority}
        render={(row) =>
          `${labelOf(AUTHORITY_KINDS, row.kind)}, document ${orNotRecorded(row.documentRef)}`
        }
      />
      <EvidenceList title="Diagnoses" rows={evidence.diagnoses} render={diagnosisText} />
      <EvidenceList title="Medications" rows={evidence.medications} render={medicationText} />
      <EvidenceList
        title="Point-of-care results (context only, not laboratory results)"
        rows={evidence.pointOfCareResults}
        render={pointOfCareText}
      />
    </Section>
  );
}

interface EvidenceListProps<Row> {
  title: string;
  rows: readonly Row[];
  render: (row: Row) => ReactNode;
}

function EvidenceList<Row extends Versioned>({ title, rows, render }: EvidenceListProps<Row>) {
  return (
    <div className={styles.group}>
      <h3 className={styles.legend}>{title}</h3>
      {rows.length === 0 ? (
        <p className={styles.hint}>None supplied.</p>
      ) : (
        <ul className={styles.list}>
          {rows.map((row) => (
            <li key={row.evidenceId}>
              {render(row)}{" "}
              <span className={styles.hint}>
                {row.removedInVersion === null
                  ? `(added in version ${row.addedInVersion})`
                  : `(removed in version ${row.removedInVersion})`}
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function diagnosisText(row: Evidence["diagnoses"][number]): string {
  const system = [row.codeSystem, row.codeSystemVersion].filter(Boolean).join(" ");
  const form = row.preprintedOnLabForm ? ", pre-printed on the laboratory's form" : "";
  return `${row.code} (${system}), from ${row.documentRef}${form}`;
}

function medicationText(row: Evidence["medications"][number]): string {
  const reporter = row.reporter ? ` (${row.reporter})` : "";
  // REQ-ORD-007: a list without an as-of date says so; it is never assumed current.
  const asOf = row.asOfDate ? `as of ${row.asOfDate}` : "as-of date unknown";
  return `${row.receivedText}, reported by ${labelOf(MEDICATION_SOURCES, row.source)}${reporter}, ${asOf}`;
}

export function pointOfCareText(row: Evidence["pointOfCareResults"][number]): string {
  const performed = row.performedAtText ? `, performed ${row.performedAtText}` : "";
  return `${row.deviceOrTest}: ${row.analyteText} ${row.resultText}, reported by ${row.source}${performed}`;
}
