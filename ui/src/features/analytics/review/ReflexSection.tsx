import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { ReflexTriggerView } from "@/generated/ReflexTriggerView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { LIST_REFLEX_TRIGGERS, ReflexDecisionDialog } from "./ReflexDecisionDialog";

/**
 * Reflex triggers of this specimen's tests with no decision yet (REQ-ORD-003, REQ-RPT-005):
 * while one is pending the report can only be partial. The engine lists them and says when a
 * decision can be recorded; a reviewer then records whether the condition is met.
 */
export function ReflexSection({ results }: { results: SpecimenResults }) {
  const triggers = useCommandQuery<ReflexTriggerView[]>(LIST_REFLEX_TRIGGERS, {
    input: { specimenId: results.specimenId },
  });
  const [deciding, setDeciding] = useState<ReflexTriggerView | null>(null);
  return (
    <Section title="Reflex decisions">
      <QueryContent query={triggers} what="reflex decisions">
        {(pending) =>
          pending.length === 0 ? (
            <p className={styles.hint}>No reflex decision is pending.</p>
          ) : (
            <ul className={styles.list} aria-label="Reflex decisions pending">
              {pending.map((trigger) => (
                <li key={`${trigger.parentItemId}:${trigger.orderableId}`}>
                  <Toolbar
                    hint={
                      trigger.allowedActions.includes("record_reflex_decision")
                        ? undefined
                        : "Approve a result in technical review before deciding."
                    }
                  >
                    <span>
                      {trigger.orderableName} ({trigger.orderableCode}): decision pending
                    </span>
                    {trigger.allowedActions.includes("record_reflex_decision") && (
                      <Button onPress={() => setDeciding(trigger)}>Record reflex decision</Button>
                    )}
                  </Toolbar>
                </li>
              ))}
            </ul>
          )
        }
      </QueryContent>
      <DataGrid
        label="Completed reflex decisions"
        rows={results.reflexHistory ?? []}
        getRowId={(r) => r.assessment.assessmentId}
        columns={[
          {
            id: "test",
            header: "Reflex test",
            cell: (r) => `${r.orderableCode} ${r.orderableName}`,
          },
          {
            id: "outcome",
            header: "Decision",
            cell: (r) => r.assessment.outcome.replaceAll("_", " "),
          },
          {
            id: "evidence",
            header: "Evidence used",
            cell: (r) => {
              const revision = results.lines
                .flatMap((l) => l.history)
                .find((v) => v.revisionId === r.assessment.triggerRef);
              return (
                <>
                  {r.assessment.triggerSource.replaceAll("_", " ")}:{" "}
                  {revision ? `result revision ${revision.revisionNo}` : r.assessment.triggerRef}
                </>
              );
            },
          },
          { id: "reason", header: "Reason", cell: (r) => r.assessment.reason },
          {
            id: "at",
            header: "Decided",
            cell: (r) => (
              <>
                <Timestamp value={r.assessment.decidedAt} /> by {r.assessment.decidedBy}
              </>
            ),
          },
          {
            id: "work",
            header: "Subsequent work",
            cell: (r) => r.createdItemState?.replaceAll("_", " ") ?? "No test activated",
          },
        ]}
      />
      {deciding && (
        <ReflexDecisionDialog
          trigger={deciding}
          lines={results.lines}
          onClose={() => setDeciding(null)}
        />
      )}
    </Section>
  );
}
