import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { LotView } from "@/generated/LotView";
import type { PrepBatchView } from "@/generated/PrepBatchView";
import type { RecordPrepStepInput } from "@/generated/RecordPrepStepInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { omittedIfBlank } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_LOTS, PREP_QUERIES } from "../contract";
import { lotText } from "../labels";

interface RecordPrepStepDialogProps {
  batch: PrepBatchView;
  onDone: (batch: PrepBatchView) => void;
  onClose: () => void;
}

/** One step as performed; the lot, if any, must be active and unexpired (engine-checked). */
export function RecordPrepStepDialog({ batch, onDone, onClose }: RecordPrepStepDialogProps) {
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: true } });
  const options = (lots.data ?? []).map((lot) => ({
    id: lot.lotId,
    label: lotText(lot),
  }));
  return (
    <CommandDialog<
      { stepCode: string; lotId: string | null; note: string },
      RecordPrepStepInput,
      PrepBatchView
    >
      title="Record preparation step"
      submitLabel="Record step"
      command={{ name: "record_prep_step", invalidates: PREP_QUERIES }}
      defaultValues={{ stepCode: "", lotId: null, note: "" }}
      toInput={(values) => ({
        prepBatchId: batch.prepBatchId,
        stepCode: values.stepCode.trim(),
        lotId: values.lotId ?? undefined,
        note: omittedIfBlank(values.note),
      })}
      successTitle="Preparation step recorded"
      onDone={onDone}
      onClose={onClose}
    >
      <TextInput
        name="stepCode"
        label="Step code"
        description="As the procedure names it."
        isRequired
      />
      <ChoiceInput name="lotId" label="Lot used" options={options} />
      <TextInput name="note" label="Note" />
    </CommandDialog>
  );
}
