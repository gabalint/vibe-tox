import { CheckList, ChoiceInput } from "@/components/FormFields";
import type { CreateRunInput } from "@/generated/CreateRunInput";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LotView } from "@/generated/LotView";
import type { RunDetail } from "@/generated/RunDetail";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_INSTRUMENT_READINESS, LIST_LOTS, RUN_QUERIES } from "../contract";
import { instrumentStatus, lotText, MATERIAL_KINDS } from "../labels";

interface CreateRunDialogProps {
  methodVersionId: string;
  onCreated: (run: RunDetail) => void;
  onClose: () => void;
}

/**
 * Creates a run of one method version on one instrument with the lots in use (REQ-ANA-001).
 * The engine refuses an instrument out of service, a lot that is expired or withdrawn, or a
 * method version that is not approved, and lists why (REQ-ANA-007).
 */
export function CreateRunDialog({ methodVersionId, onCreated, onClose }: CreateRunDialogProps) {
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: true } });
  const instrumentOptions = (instruments.data ?? []).map((instrument) => ({
    id: instrument.instrumentId,
    label: `${instrument.name}: ${instrumentStatus(instrument.status).label.toLowerCase()}`,
  }));
  const lotOptions = (lots.data ?? []).map((lot) => ({
    id: lot.lotId,
    label: `${labelOf(MATERIAL_KINDS, lot.materialKind)}: ${lotText(lot)}`,
    details: [`Expires ${lot.expiresOn}`],
  }));
  return (
    <CommandDialog<{ instrumentId: string | null; lotIds: string[] }, CreateRunInput, RunDetail>
      title="Create run"
      submitLabel="Create run"
      command={{ name: "create_run", invalidates: RUN_QUERIES }}
      defaultValues={{ instrumentId: null, lotIds: [] }}
      toInput={(values) => ({
        instrumentId: checked(values.instrumentId),
        methodVersionId,
        lotIds: values.lotIds,
      })}
      successTitle="Run created"
      onDone={onCreated}
      onClose={onClose}
    >
      <ChoiceInput name="instrumentId" label="Instrument" options={instrumentOptions} isRequired />
      <CheckList name="lotIds" legend="Lots used on this run" options={lotOptions} />
    </CommandDialog>
  );
}
