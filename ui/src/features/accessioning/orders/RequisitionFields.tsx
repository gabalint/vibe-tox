import { useContext, useMemo } from "react";
import { FormValidationContext } from "react-aria-components";
import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, RecordPickerInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import type { ClientView } from "@/generated/ClientView";
import type { PractitionerView } from "@/generated/PractitionerView";
import { YES_NO } from "@/lib/choices";
import { BILLING_RESPONSIBILITIES, SOURCE_KINDS, TEMPLATE_KINDS } from "./labels";
import styles from "./OrderForm.module.css";
import type { OrderFormValues } from "./orderValues";
import { practitionerOption } from "./references";

const clientOption = (client: ClientView) => ({
  id: client.id,
  label: client.name,
  description: client.code,
});

interface RequisitionFieldsProps {
  clients: readonly ClientView[];
  practitioners: readonly PractitionerView[];
}

/**
 * Who ordered, for whom the results are, and where the order came from (REQ-ORD-001). The
 * client and practitioner are chosen by typing part of a name, code or NPI; nothing is
 * chosen in advance (REQ-ORD-005). The location lists only the chosen client's locations.
 */
export function RequisitionFields({ clients, practitioners }: RequisitionFieldsProps) {
  const { control, setValue } = useFormContext<OrderFormValues>();
  const errors = useContext(FormValidationContext);
  const referencesInvalid = [
    "placerOrderId",
    "placerAuthority",
    "sourceDocumentRef",
    "facilitySetting",
  ].some((name) => errors[name]);
  const [clientId, templateKind] = useWatch({ control, name: ["clientId", "templateKind"] });
  // The lists can be long (NFR-33): build their options once, not on every keystroke.
  const clientOptions = useMemo(() => clients.map(clientOption), [clients]);
  const practitionerOptions = useMemo(() => practitioners.map(practitionerOption), [practitioners]);
  // No client yet: no location to choose.
  const locations = clientId
    ? (clients.find((client) => client.id === clientId)?.locations ?? [])
    : null;
  return (
    <Section title="Ordering source">
      <div className={styles.fields}>
        <RecordPickerInput
          name="clientId"
          label="Client"
          options={clientOptions}
          onChange={() => setValue("clientLocationId", null)}
          isRequired
        />
        <ClientLocationField locations={locations} />
        <RecordPickerInput
          name="practitionerId"
          label="Ordering practitioner"
          options={practitionerOptions}
          onChange={() => setValue("standingOrderId", null)}
          isRequired
        />
        <ChoiceInput name="sourceKind" label="Received as" options={SOURCE_KINDS} isRequired />
        <ChoiceInput
          name="billingResponsibility"
          label="Billing responsibility"
          options={BILLING_RESPONSIBILITIES}
          isRequired
        />
        <ChoiceInput
          name="templateKind"
          label="Template use"
          options={TEMPLATE_KINDS}
          description="Whether the clinic marked tests on a preset template."
          isRequired
        />
        {templateKind === "clinic_template" && (
          <>
            <TextInput name="templateName" label="Template name" isRequired />
            <ChoiceInput
              name="templateConfirmed"
              label="Confirmed for this patient"
              options={YES_NO}
              description="The practitioner confirmed the template's tests for this patient."
              isRequired
            />
          </>
        )}
      </div>
      <details className={styles.references} open={referencesInvalid || undefined}>
        <summary>Clinic order number and source references</summary>
        <div className={styles.fields}>
          <TextInput name="placerOrderId" label="Clinic order number" />
          <TextInput
            name="placerAuthority"
            label="Issued by"
            description="The clinic or system that numbered the order."
          />
          <TextInput
            name="sourceDocumentRef"
            label="Source document reference"
            description="Such as the scanned requisition's number."
          />
          <TextInput name="facilitySetting" label="Facility setting" />
        </div>
      </details>
    </Section>
  );
}

/**
 * Where results go, among the chosen client's locations; `null` before a client is chosen,
 * when there is nothing to choose, so the field is disabled.
 */
function ClientLocationField({ locations }: { locations: ClientView["locations"] | null }) {
  return (
    <ChoiceInput
      name="clientLocationId"
      label="Client location"
      options={(locations ?? []).map((location) => ({ id: location.id, label: location.name }))}
      isDisabled={locations === null}
      description={
        locations === null
          ? "Choose the client first."
          : "Where results go, when the requisition names a location."
      }
    />
  );
}
