import { useState } from "react";
import { Button } from "@/components/Button";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import type { ValidityAssessmentView } from "@/generated/ValidityAssessmentView";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { useCommandQuery } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import { AssessmentHistory } from "../AssessmentHistory";
import { GET_VALIDITY_POLICY } from "../contract";
import { ValidityDialog } from "./ValidityDialog";

/**
 * Specimen validity (REQ-SPC-004, REQ-SPC-008): when the tests include validity checks, no
 * drug result can be reviewed until a reviewer records the category under an approved
 * policy. A new assessment replaces the current one with a reason.
 */
export function ValiditySection({ results }: { results: SpecimenResults }) {
  const [isAssessing, setAssessing] = useState(false);
  return (
    <Section
      title="Specimen validity"
      actions={<Button onPress={() => setAssessing(true)}>Record validity assessment</Button>}
    >
      {isAssessing && <ValidityDialog results={results} onClose={() => setAssessing(false)} />}
      <p>
        {results.validityRequired
          ? "These tests include validity checks: an assessment is needed before review."
          : "These tests include no validity checks."}
      </p>
      <AssessmentHistory
        current={results.currentValidity}
        history={results.validityHistory}
        getId={(item) => item.assessmentId}
        render={(item) => <Assessment item={item} />}
        emptyText="Not assessed."
      />
    </Section>
  );
}

/** The category's label comes from the policy version it was assessed under. */
function Assessment({ item }: { item: ValidityAssessmentView }) {
  const person = usePersonText();
  const policy = useCommandQuery<ValidityPolicyView>(GET_VALIDITY_POLICY, {
    input: { policyVersionId: item.policyVersionId },
  });
  const category = policy.data?.categories.find(
    (candidate) => candidate.code === item.categoryCode,
  );
  const version = policy.data ? `, policy version ${policy.data.version}` : "";
  return (
    <p>
      {category?.label ?? item.categoryCode}
      {version}. <Timestamp value={item.assessedAt} /> by{" "}
      {person(item.assessedBy, item.assessedByDisplay)}
      {item.reason ? `. Reason: ${item.reason}` : ""}
    </p>
  );
}
