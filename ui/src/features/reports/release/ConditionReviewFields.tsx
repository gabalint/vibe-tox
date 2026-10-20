import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ConditionReviewSource } from "@/generated/ConditionReviewSource";
import { labelOf } from "@/lib/choices";
import { useFormatTime } from "@/lib/format";
import { DISCREPANCIES, RECEIPT_DECISIONS } from "../../accessioning/specimens/labels";

/** Explicit report disclosure decisions; source receipt evidence stays unchanged. */
export function ConditionReviewFields({ source }: { source: ConditionReviewSource }) {
  const formatTime = useFormatTime();
  return (
    <fieldset>
      <legend>Receipt condition review</legend>
      <p>
        These are internal receipt records. Include only clinically relevant condition information
        in the patient report. Current condition codes and disposition always remain on the report.
        Every original note remains in the laboratory record, including excluded notes.
      </p>
      {source.conditions.map((condition, index) => {
        const current = condition.decisionId === source.latestDecisionId;
        return (
          <fieldset key={`${condition.decisionId}:${condition.code}`}>
            <legend>
              {current ? "Current" : "Earlier"} condition {index + 1}:{" "}
              {labelOf(DISCREPANCIES, condition.code)}
            </legend>
            <p>
              {labelOf(RECEIPT_DECISIONS, condition.decision)} · {formatTime(condition.recordedAt)}
            </p>
            <p>{condition.description ?? "No descriptive note was recorded."}</p>
            <ChoiceInput
              name={`conditionReview.choices.${index}`}
              label={`Report choice for condition ${index + 1}`}
              options={[
                {
                  id: "include",
                  label: current
                    ? "Include description"
                    : "Include earlier condition and description",
                },
                {
                  id: "exclude",
                  label: current
                    ? "Keep description internal; report current code"
                    : "Keep earlier condition internal",
                },
              ]}
              description={
                current
                  ? "The current code and receipt disposition cannot be excluded."
                  : "Select whether this earlier receipt condition is relevant to the patient report."
              }
              isRequired
            />
          </fieldset>
        );
      })}
      <TextInput
        name="conditionReview.reason"
        label="Reason for condition selections"
        description="Required, up to 1,000 characters. Retained with the candidate for audit; not printed on the patient report."
        isRequired
      />
    </fieldset>
  );
}
