import { useState } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { AddPatientPositionInput } from "@/generated/AddPatientPositionInput";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { RunDetail } from "@/generated/RunDetail";
import type { RunPositionView } from "@/generated/RunPositionView";
import type { RunSummary } from "@/generated/RunSummary";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { useCommandPages, useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { LIST_INSTRUMENT_READINESS, LIST_RUNS, RUN_QUERIES } from "../contract";
import { ATTEMPT_KINDS } from "../labels";
import { CreateRunDialog } from "./CreateRunDialog";
import { PrepBatchChoice } from "./PrepBatchChoice";

interface Values {
  runId: string | null;
  positionLabel: string;
  containerLabel: string;
  attemptKind: string | null;
  dilutionFactor: string;
  prepBatchId: string | null;
  reason: string;
}

/** A repeat belongs to the exact original execution and method; the engine rechecks both. */
export function RepeatPositionDialog({
  run,
  position,
  onClose,
}: {
  run: RunDetail;
  position: RunPositionView;
  onClose: () => void;
}) {
  const [creating, setCreating] = useState(false);
  const [createdRun, setCreatedRun] = useState<string | null>(null);
  if (!position.execution) return null;
  if (creating)
    return (
      <CreateRunDialog
        methodVersionId={run.methodVersionId}
        onCreated={(created) => {
          setCreatedRun(created.runId);
          setCreating(false);
        }}
        onClose={() => setCreating(false)}
      />
    );
  return (
    <CommandDialog<Values, AddPatientPositionInput, RunDetail>
      title="Repeat test"
      submitLabel="Record repeat"
      command={{ name: "add_patient_position", invalidates: RUN_QUERIES }}
      defaultValues={{
        runId: createdRun,
        positionLabel: "",
        containerLabel: position.sampleLabel,
        attemptKind: null,
        dilutionFactor: "",
        prepBatchId: null,
        reason: "",
      }}
      toInput={(values) => ({
        runId: checked(values.runId),
        positionLabel: values.positionLabel.trim(),
        containerLabel: values.containerLabel.trim(),
        attemptKind: checked(choiceOf(ATTEMPT_KINDS, values.attemptKind)),
        relatedExecutionId: position.execution?.executionId,
        dilutionFactor:
          values.attemptKind === "dilution" ? values.dilutionFactor.trim() : undefined,
        prepBatchId: values.prepBatchId ?? undefined,
        reason: values.reason.trim(),
      })}
      successTitle="Repeat recorded"
      onClose={onClose}
    >
      <p>
        Follows position {position.label}, {position.sampleLabel}, under {run.methodCode} version{" "}
        {run.methodVersion}. Original observations remain unchanged.
      </p>
      <OpenRunChoice methodVersionId={run.methodVersionId} />
      <Button onPress={() => setCreating(true)}>Create a run for this version</Button>
      <TextInput name="positionLabel" label="Position" isRequired />
      <TextInput
        name="containerLabel"
        label="Container barcode"
        isRequired
        description="Original material is shown. A replacement aliquot must belong to the same specimen."
      />
      <ChoiceInput
        name="attemptKind"
        label="Attempt kind"
        isRequired
        options={ATTEMPT_KINDS.filter((kind) => kind.id !== "initial")}
      />
      <DilutionField />
      <PrepBatchChoice methodVersionId={run.methodVersionId} />
      <TextInput
        name="reason"
        label="Reason"
        isRequired
        description="Why the repeat was performed. For an omitted preparation link, repeat with the correct completed batch; historical attempts are retained."
      />
    </CommandDialog>
  );
}

function DilutionField() {
  const { control } = useFormContext<Values>();
  const kind = useWatch({ control, name: "attemptKind" });
  return kind === "dilution" ? (
    <TextInput
      name="dilutionFactor"
      label="Dilution factor"
      isRequired
      description="Positive exact decimal, recorded as entered. The source must report final concentration; the LIS does not multiply it."
    />
  ) : null;
}

function OpenRunChoice({ methodVersionId }: { methodVersionId: string }) {
  const formatTime = useFormatTime();
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const runs = useCommandPages<RunSummary, string>(LIST_RUNS, {
    key: { state: "open", methodVersionId },
    pageArgs: (afterRunId) => ({
      input: { state: "open", methodVersionId, afterRunId, limit: 50 },
    }),
    nextCursor: (page) => (page.length === 50 ? page.at(-1)?.runId : undefined),
  });
  return (
    <QueryContent query={runs} what="open runs">
      {(data) => (
        <>
          <ChoiceInput
            name="runId"
            label="Target run"
            isRequired
            description="Only open runs using the original method version are eligible."
            options={data.pages
              .flat()
              .filter((row) => row.methodVersionId === methodVersionId)
              .map((row) => ({
                id: row.runId,
                label: `${formatTime(row.createdAt)} · ${instruments.data?.find((i) => i.instrumentId === row.instrumentId)?.name ?? row.instrumentId} · ${row.positionCount} positions`,
              }))}
          />
          {runs.hasNextPage && (
            <Button isPending={runs.isFetchingNextPage} onPress={() => void runs.fetchNextPage()}>
              Load more open runs
            </Button>
          )}
        </>
      )}
    </QueryContent>
  );
}
