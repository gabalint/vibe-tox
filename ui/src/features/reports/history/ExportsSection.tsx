import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import type { ReportDetail } from "../contract";
import { EXPORT_PURPOSES } from "../labels";

/**
 * PDF exports of the report's revisions and notices, each recorded and none of them a
 * delivery (NFR-07). The engine words each one (REQ-RPT-011).
 */
export function ExportsSection({ exports }: { exports: ReportDetail["exports"] }) {
  return (
    <Section title="PDF exports">
      {exports.length === 0 ? (
        <p className={styles.hint}>Not exported.</p>
      ) : (
        <ul aria-label="PDF exports" className={styles.list}>
          {exports.map((each) => (
            <li key={each.exportId}>
              {each.label}: {each.fileName}, <Timestamp value={each.exportedAt} />.{" "}
              {EXPORT_PURPOSES.find((purpose) => purpose.id === each.purpose)?.label}.
            </li>
          ))}
        </ul>
      )}
    </Section>
  );
}
