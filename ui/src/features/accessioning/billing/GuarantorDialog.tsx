import { useWatch } from "react-hook-form";
import { FieldGrid, FieldSection } from "@/components/Form";
import { ChoiceInput } from "@/components/FormFields";
import { partyInput, partyValues } from "./contactValues";
import { PartyFields } from "./FinancialFields";
import { PatientProfileDialog, type ProfileDialogProps } from "./PatientProfileDialog";

const PARTY_STATUS = [
  { id: "recorded", label: "Responsible party supplied" },
  { id: "not_supplied", label: "Not supplied" },
];

export function GuarantorDialog(props: ProfileDialogProps) {
  const guarantor = props.view.profile?.content.guarantor;
  return (
    <PatientProfileDialog
      {...props}
      title="Guarantor or responsible party"
      defaultValues={{
        status: guarantor ? "recorded" : null,
        party: partyValues(guarantor),
        source: "",
        reason: "",
      }}
      update={(content, values) => ({
        ...content,
        guarantor: values.status === "recorded" ? partyInput(values.party) : null,
      })}
    >
      <FieldSection title="Responsible party">
        <FieldGrid>
          <ChoiceInput
            name="status"
            label="Responsible party details"
            options={PARTY_STATUS}
            isRequired
          />
        </FieldGrid>
        <GuarantorFields />
      </FieldSection>
    </PatientProfileDialog>
  );
}

function GuarantorFields() {
  const status = useWatch({ name: "status" });
  return status === "recorded" ? <PartyFields prefix="party" label="Guarantor" /> : null;
}
