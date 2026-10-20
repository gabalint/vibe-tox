import { contactInput, contactValues } from "./contactValues";
import { ContactFields } from "./FinancialFields";
import { PatientProfileDialog, type ProfileDialogProps } from "./PatientProfileDialog";

export function ContactDialog(props: ProfileDialogProps) {
  return (
    <PatientProfileDialog
      {...props}
      title="Patient billing address and contact"
      defaultValues={{
        contact: contactValues(props.view.profile?.content.contact),
        source: "",
        reason: "",
      }}
      update={(content, values) => ({ ...content, contact: contactInput(values.contact) })}
    >
      <ContactFields prefix="contact" />
    </PatientProfileDialog>
  );
}
