import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { DeviationTarget } from "@/generated/DeviationTarget";
import type { DeviationView } from "@/generated/DeviationView";
import type { OpenDeviationInput } from "@/generated/OpenDeviationInput";
import type { RunQcView } from "@/generated/RunQcView";
import { CommandDialog } from "@/lib/CommandDialog";
import { type Choice, checked, choiceOf } from "@/lib/choices";
import { QC_QUERIES } from "../contract";

/** What a deviation can be opened against from this run's QC. */
export interface QcTargets {
  positions: { id: string; label: string }[];
  lots: { id: string; label: string }[];
  analytes: { id: string; label: string }[];
}

/** The positions and lots that reported QC on this run, and its analytes. */
export function qcTargets(view: RunQcView, lotName: (lotId: string | null) => string): QcTargets {
  const observations = [
    ...view.analytes.flatMap((analyte) => analyte.observations),
    ...view.unmappedObservations,
  ];
  const positions = new Map(observations.map((row) => [row.positionId, row.positionLabel]));
  const lotIds = new Set(observations.flatMap((row) => (row.lotId === null ? [] : [row.lotId])));
  return {
    positions: [...positions].map(([id, label]) => ({ id, label: `Position ${label}` })),
    lots: [...lotIds].map((id) => ({ id, label: lotName(id) })),
    analytes: view.analytes.map((analyte) => ({ id: analyte.analyteId, label: analyte.code })),
  };
}

type TargetKind = "run" | "position" | "lot";

const KINDS: readonly Choice<TargetKind>[] = [
  { id: "run", label: "This run" },
  { id: "position", label: "One position" },
  { id: "lot", label: "One lot" },
];

interface DeviationValues {
  kind: string | null;
  targetId: string | null;
  analyteId: string | null;
  description: string;
  reason: string;
}

function toTarget(runId: string, values: DeviationValues): DeviationTarget {
  const kind = checked(choiceOf(KINDS, values.kind));
  if (kind === "run") return { kind, id: runId };
  return { kind, id: checked(values.targetId) };
}

interface OpenDeviationDialogProps {
  runId: string;
  targets: QcTargets;
  onClose: () => void;
}

/** Records a deviation against the run, one of its QC positions or one of its lots. */
export function OpenDeviationDialog({ runId, targets, onClose }: OpenDeviationDialogProps) {
  return (
    <CommandDialog<DeviationValues, OpenDeviationInput, DeviationView>
      title="Open deviation"
      submitLabel="Open deviation"
      command={{ name: "open_deviation", invalidates: QC_QUERIES }}
      defaultValues={{ kind: null, targetId: null, analyteId: null, description: "", reason: "" }}
      toInput={(values) => ({
        scope: toTarget(runId, values),
        analyteId: values.analyteId ?? undefined,
        description: values.description.trim(),
        reason: values.reason.trim(),
      })}
      successTitle="Deviation opened"
      onClose={onClose}
    >
      <ChoiceInput name="kind" label="Applies to" options={KINDS} isRequired />
      <TargetChoice targets={targets} />
      <ChoiceInput name="analyteId" label="Analyte" options={targets.analytes} />
      <TextInput name="description" label="Description" isRequired />
      <ReasonField />
    </CommandDialog>
  );
}

function TargetChoice({ targets }: { targets: QcTargets }) {
  const { control } = useFormContext<DeviationValues>();
  const kind = choiceOf(KINDS, useWatch({ control, name: "kind" }));
  if (kind === "position") {
    return <ChoiceInput name="targetId" label="Position" options={targets.positions} isRequired />;
  }
  if (kind === "lot") {
    return <ChoiceInput name="targetId" label="Lot" options={targets.lots} isRequired />;
  }
  return null;
}
