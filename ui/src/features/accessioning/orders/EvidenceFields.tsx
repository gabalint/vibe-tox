import { ChoiceInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { DiagnosisList } from "./DiagnosisList";
import { AUTHORITY_KINDS } from "./labels";
import { MedicationList } from "./MedicationList";
import styles from "./OrderForm.module.css";
import { PocResultList } from "./PocResultList";

/**
 * Evidence exactly as the ordering source supplied it (REQ-ORD-005, REQ-ORD-007): the
 * authority to test, diagnoses with their source document, medications and point-of-care
 * results. Nothing is suggested, completed or inferred.
 */
export function EvidenceFields() {
  return (
    <Section title="Evidence as supplied">
      <div className={styles.fields}>
        <ChoiceInput
          name="authority.kind"
          label="Authority to test"
          options={AUTHORITY_KINDS}
          isRequired
        />
        <TextInput name="authority.documentRef" label="Authority document reference" />
      </div>
      <DiagnosisList />
      <MedicationList />
      <PocResultList />
    </Section>
  );
}
