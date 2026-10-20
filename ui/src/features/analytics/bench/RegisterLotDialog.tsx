import { ChoiceInput, DATE_FORMAT, TextInput } from "@/components/FormFields";
import type { LotView } from "@/generated/LotView";
import type { RegisterLotInput } from "@/generated/RegisterLotInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, omittedIfBlank } from "@/lib/choices";
import { LOT_QUERIES } from "../contract";
import { MATERIAL_KINDS } from "../labels";

interface LotValues {
  materialKind: string | null;
  productName: string;
  lotNumber: string;
  levelText: string;
  sampleLabel: string;
  expiresOn: string;
}

interface RegisterLotDialogProps {
  /** Receives the new lot, to open it. */
  onDone: (lot: LotView) => void;
  onClose: () => void;
}

/** Records a lot as printed on its packaging (REQ-ANA-007). */
export function RegisterLotDialog({ onDone, onClose }: RegisterLotDialogProps) {
  return (
    <CommandDialog<LotValues, RegisterLotInput, LotView>
      title="Register lot"
      submitLabel="Register lot"
      command={{ name: "register_lot", invalidates: LOT_QUERIES }}
      defaultValues={{
        materialKind: null,
        productName: "",
        lotNumber: "",
        levelText: "",
        sampleLabel: "",
        expiresOn: "",
      }}
      toInput={(values) => ({
        materialKind: checked(choiceOf(MATERIAL_KINDS, values.materialKind)),
        productName: values.productName.trim(),
        lotNumber: values.lotNumber.trim(),
        levelText: omittedIfBlank(values.levelText),
        sampleLabel: omittedIfBlank(values.sampleLabel),
        expiresOn: values.expiresOn.trim(),
      })}
      successTitle="Lot registered"
      onDone={onDone}
      onClose={onClose}
    >
      <ChoiceInput name="materialKind" label="Material" options={MATERIAL_KINDS} isRequired />
      <TextInput name="productName" label="Product" isRequired />
      <TextInput name="lotNumber" label="Lot number" isRequired />
      <TextInput name="levelText" label="Level" description="Such as Level 1, for controls." />
      <TextInput
        name="sampleLabel"
        label="Sample label"
        description="The label the analyzer reports for this control or calibrator."
      />
      <TextInput name="expiresOn" label="Expires on" format={DATE_FORMAT} isRequired />
    </CommandDialog>
  );
}
