import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import type { CompletePrepBatchInput } from "@/generated/CompletePrepBatchInput";
import type { CreatePrepBatchInput } from "@/generated/CreatePrepBatchInput";
import type { PrepBatchView } from "@/generated/PrepBatchView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { GET_PREP_BATCH, PREP_QUERIES } from "../contract";
import { PrepBatchDetail } from "./PrepBatchDetail";
import { PrepBatchDirectory } from "./PrepBatchDirectory";
import { RecordPrepStepDialog } from "./RecordPrepStepDialog";

/**
 * A preparation batch for this method version (REQ-ANA-001): start it, record each step, then
 * complete it. Completion fixes the steps for good; QC can accept a batch only once it is
 * completed. Saved batches remain reachable through the directory and address.
 */
export function PrepBatchSection({ methodVersionId }: { methodVersionId: string }) {
  const [params, setParams] = useSearchParams();
  const client = useQueryClient();
  const batchId = params.get("batch");
  const query = useCommandQuery<PrepBatchView>(
    GET_PREP_BATCH,
    { input: { prepBatchId: batchId } },
    { enabled: Boolean(batchId) },
  );
  const batch = query.data?.methodVersionId === methodVersionId ? query.data : undefined;
  const open = (id: string) =>
    setParams((current) => {
      const next = new URLSearchParams(current);
      next.set("batch", id);
      return next;
    });
  const setBatch = (saved: PrepBatchView) => {
    client.setQueryData([GET_PREP_BATCH, { input: { prepBatchId: saved.prepBatchId } }], saved);
    open(saved.prepBatchId);
  };
  const [dialog, setDialog] = useState<"create" | "step" | "complete" | null>(null);
  const close = () => setDialog(null);
  const isOpen = batch?.state === "open";
  return (
    <Section
      title="Preparation batch"
      actions={
        isOpen ? (
          <>
            <Button onPress={() => setDialog("step")}>Record step</Button>
            <Button onPress={() => setDialog("complete")}>Complete batch</Button>
          </>
        ) : (
          <Button onPress={() => setDialog("create")}>Start preparation batch</Button>
        )
      }
    >
      <p className={styles.hint}>
        QC can accept a batch only once it is completed. Open a saved batch below to resume its
        steps after navigation or restart. Choose the completed batch when placing samples.
      </p>
      <PrepBatchDirectory methodVersionId={methodVersionId} onOpen={open} />
      {batchId && (
        <>
          <RefreshControl query={query} />
          <QueryContent query={query} what="preparation batch">
            {(loaded) =>
              loaded.methodVersionId === methodVersionId ? (
                <PrepBatchDetail batch={loaded} />
              ) : (
                <EmptyState
                  title="Batch belongs to another method version"
                  description="Open a batch from the directory for the selected version. The saved batch and its steps remain unchanged."
                />
              )
            }
          </QueryContent>
        </>
      )}
      {dialog === "create" && (
        <CommandConfirmDialog<CreatePrepBatchInput, PrepBatchView>
          title="Start a preparation batch?"
          confirmLabel="Start batch"
          variant="primary"
          command={{ name: "create_prep_batch", invalidates: PREP_QUERIES }}
          input={{ methodVersionId }}
          successTitle="Preparation batch started"
          onDone={setBatch}
          onClose={close}
        >
          <p>The batch is recorded for the method version chosen above.</p>
        </CommandConfirmDialog>
      )}
      {dialog === "step" && batch && (
        <RecordPrepStepDialog batch={batch} onDone={setBatch} onClose={close} />
      )}
      {dialog === "complete" && batch && (
        <CommandConfirmDialog<CompletePrepBatchInput, PrepBatchView>
          title="Complete the preparation batch?"
          confirmLabel="Complete batch"
          variant="primary"
          command={{ name: "complete_prep_batch", invalidates: PREP_QUERIES }}
          input={{ prepBatchId: batch.prepBatchId, expectedRevision: batch.revision }}
          successTitle="Preparation batch completed"
          onDone={setBatch}
          onClose={close}
        >
          <p>
            {`Completes the batch with the ${batch.steps.length} ${batch.steps.length === 1 ? "step" : "steps"} shown. No step can be added or changed afterwards.`}
          </p>
        </CommandConfirmDialog>
      )}
    </Section>
  );
}
