import { Button } from "@/components/Button";
import { ChoiceInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { PrepBatchSummary } from "@/generated/PrepBatchSummary";
import { useCommandPages } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { LIST_PREP_BATCHES } from "../contract";

export function usePrepBatches(methodVersionId: string) {
  return useCommandPages<PrepBatchSummary, string>(LIST_PREP_BATCHES, {
    key: { methodVersionId },
    pageArgs: (after) => ({ input: { methodVersionId, after: after ?? null, limit: 50 } }),
    nextCursor: (page) => (page.length === 50 ? page.at(-1)?.prepBatchId : undefined),
  });
}

/** A completed batch for exactly the run's pinned method; nothing is inferred. */
export function PrepBatchChoice({ methodVersionId }: { methodVersionId: string }) {
  const batches = usePrepBatches(methodVersionId);
  const formatTime = useFormatTime();
  return (
    <QueryContent query={batches} what="preparation batches">
      {(data) => (
        <>
          <ChoiceInput
            name="prepBatchId"
            label="Completed preparation batch"
            description="Optional. Choose the preparation actually used; QC acceptance is checked separately."
            options={data.pages
              .flat()
              .filter((batch) => batch.state === "completed")
              .map((batch) => ({
                id: batch.prepBatchId,
                label: `${formatTime(batch.createdAt)} · ${batch.stepCount} steps · ${batch.prepBatchId}`,
              }))}
          />
          {batches.hasNextPage && (
            <Button
              isPending={batches.isFetchingNextPage}
              onPress={() => void batches.fetchNextPage()}
            >
              Load more preparation batches
            </Button>
          )}
        </>
      )}
    </QueryContent>
  );
}
