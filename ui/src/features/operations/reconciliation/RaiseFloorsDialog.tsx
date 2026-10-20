import { TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { RecoveryFloorView } from "@/generated/RecoveryFloorView";
import { CommandDialog } from "@/lib/CommandDialog";
import { omittedIfBlank } from "@/lib/choices";
import { RAISE_NUMBER_FLOORS, RECONCILIATION_QUERIES } from "../contract";

interface FloorValues {
  accessionFloor: string;
  containerFloor: string;
  reason: string;
}

/** `RaiseNumberFloorsInput` of `lis-specimens`, which is not exported to `@/generated`. */
interface FloorInput {
  accessionFloor?: string;
  containerFloor?: string;
  reason: string;
}

interface RaiseFloorsDialogProps {
  floors: RecoveryFloorView;
  onClose: () => void;
}

/**
 * Raises the next accession number or container code (design brief §3.8, L8), so new labels
 * never repeat one printed in the lost interval. The engine only lets a floor go up.
 */
export function RaiseFloorsDialog({ floors, onClose }: RaiseFloorsDialogProps) {
  return (
    <CommandDialog<FloorValues, FloorInput>
      title="Raise number floors"
      submitLabel="Raise floors"
      command={{ name: RAISE_NUMBER_FLOORS, invalidates: RECONCILIATION_QUERIES }}
      defaultValues={{ accessionFloor: "", containerFloor: "", reason: "" }}
      toInput={(values) => ({
        accessionFloor: omittedIfBlank(values.accessionFloor),
        containerFloor: omittedIfBlank(values.containerFloor),
        reason: values.reason.trim(),
      })}
      successTitle="Number floors raised"
      onClose={onClose}
    >
      <p>Leave a floor empty to keep it as it is.</p>
      <TextInput
        name="accessionFloor"
        label="New accession number floor"
        description={`Now ${floors.accession}.`}
      />
      <TextInput
        name="containerFloor"
        label="New container code floor"
        description={`Now ${floors.container}.`}
      />
      <ReasonField />
    </CommandDialog>
  );
}
