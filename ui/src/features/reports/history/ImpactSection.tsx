import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import type { ImpactView } from "../contract";
import { IMPACT_DECISIONS } from "../labels";
import { ImpactDialog } from "./ImpactDialog";

interface ImpactSectionProps {
  impact: ImpactView;
  canAssess: boolean;
}

/**
 * What changed after the current revision was released (REQ-RPT-002, REQ-RPT-013), as the
 * engine computes it from records that cannot change, and the quality decisions taken. While
 * a reason is open, delivery of the revision is paused.
 */
export function ImpactSection({ impact, canAssess }: ImpactSectionProps) {
  const [isAssessing, setAssessing] = useState(false);
  return (
    <Section
      title="Impact after release"
      actions={
        canAssess &&
        impact.open.length > 0 && (
          <Button onPress={() => setAssessing(true)}>Record impact assessment</Button>
        )
      }
    >
      {impact.correctionDue && (
        <Alert tone="warning" title="Correction due">
          A quality decision requires a corrected revision of this report.
        </Alert>
      )}
      {impact.open.length === 0 ? (
        <p className={styles.hint}>Nothing open.</p>
      ) : (
        <>
          <p>Delivery of the current revision is paused until these are assessed.</p>
          <ul aria-label="Open impact" className={styles.list}>
            {impact.open.map((item) => (
              <li key={item.key}>{item.message}</li>
            ))}
          </ul>
        </>
      )}
      {impact.assessments.map((assessment) => (
        <Facts
          key={assessment.assessmentId}
          items={[
            {
              label: "Assessment",
              value:
                IMPACT_DECISIONS.find((each) => each.id === assessment.decision)?.label ??
                assessment.decision,
            },
            { label: "Reason", value: assessment.reason },
            {
              label: "Assessed",
              value: (
                <>
                  <Timestamp value={assessment.assessedAt} /> by {assessment.assessedBy.displayName}
                </>
              ),
            },
          ]}
        />
      ))}
      {isAssessing && (
        <ImpactDialog
          revisionId={impact.revisionId}
          open={impact.open}
          onClose={() => setAssessing(false)}
        />
      )}
    </Section>
  );
}
