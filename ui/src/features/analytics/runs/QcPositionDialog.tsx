import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { AddQcPositionInput } from "@/generated/AddQcPositionInput";
import type { LotView } from "@/generated/LotView";
import type { QcRole } from "@/generated/QcRole";
import type { RunDetail } from "@/generated/RunDetail";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, labelOf, omittedIfBlank } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_LOTS, RUN_QUERIES } from "../contract";
import { lotText, MATERIAL_KINDS, QC_ROLES } from "../labels";

interface QcValues {
  positionLabel: string;
  role: string | null;
  lotId: string | null;
  sampleLabel: string;
}

/**
 * Adds a control, calibrator or blank position (REQ-ANA-001). Controls and calibrators take
 * the sample label of their lot; a blank has no lot and is given its own label. These
 * positions never become patient results.
 */
export function QcPositionDialog({ run, onClose }: { run: RunDetail; onClose: () => void }) {
  return (
    <CommandDialog<QcValues, AddQcPositionInput, RunDetail>
      title="Add control, calibrator or blank"
      submitLabel="Add position"
      command={{ name: "add_qc_position", invalidates: RUN_QUERIES }}
      defaultValues={{ positionLabel: "", role: null, lotId: null, sampleLabel: "" }}
      toInput={(values) => {
        const role = checked(choiceOf(QC_ROLES, values.role));
        return role === "blank"
          ? {
              runId: run.runId,
              positionLabel: values.positionLabel.trim(),
              role,
              sampleLabel: omittedIfBlank(values.sampleLabel),
            }
          : {
              runId: run.runId,
              positionLabel: values.positionLabel.trim(),
              role,
              lotId: values.lotId ?? undefined,
            };
      }}
      successTitle="Position added"
      onClose={onClose}
    >
      <TextInput name="positionLabel" label="Position" isRequired />
      <ChoiceInput name="role" label="Role" options={QC_ROLES} isRequired />
      <MaterialField />
    </CommandDialog>
  );
}

/** The lot for a control or calibrator, or the sample label of a blank. */
function MaterialField() {
  const { control } = useFormContext<QcValues>();
  const role = choiceOf(QC_ROLES, useWatch({ control, name: "role" }));
  if (role === null) return null;
  if (role === "blank") return <TextInput name="sampleLabel" label="Sample label" isRequired />;
  return <LotChoice role={role} />;
}

function LotChoice({ role }: { role: Exclude<QcRole, "blank"> }) {
  const lots = useCommandQuery<LotView[]>(LIST_LOTS, { input: { activeOnly: true } });
  // Lots of the role's material only; the engine checks the match again.
  const options = (lots.data ?? [])
    .filter((lot) => lot.materialKind === role)
    .map((lot) => ({
      id: lot.lotId,
      label: `${lotText(lot)}${lot.levelText ? `, ${lot.levelText}` : ""} (${lot.sampleLabel ?? "no sample label"})`,
    }));
  return (
    <ChoiceInput
      name="lotId"
      label={`${labelOf(MATERIAL_KINDS, role)} lot`}
      options={options}
      isRequired
    />
  );
}
