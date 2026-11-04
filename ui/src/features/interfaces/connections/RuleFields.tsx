import { X } from "lucide-react";
import { useFieldArray, useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import row from "../actionRow.module.css";
import { RULE_CONDITIONS, ruleFields } from "../labels";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

const CONDITION_OPTIONS = Object.entries(RULE_CONDITIONS).map(([id, label]) => ({ id, label }));
const MAX_RULES = 20;

/**
 * Section 4, the routing rules: which messages go to this connection, one rule per line
 * (field, condition, value). Every rule must match, for every value of its field in the
 * message; no rules means every message of the type. An HL7 connection names the fields by
 * the HL7 fields that carry them.
 */
export function RuleFields() {
  const { control } = useFormContext<ConnectionValues>();
  const rules = useFieldArray({ control, name: "rules" });
  const messageType = useWatch({ control, name: "messageType" }) as PartnerMessageType | "";
  const fieldOptions = Object.entries(ruleFields(messageType)).map(([id, label]) => ({
    id,
    label,
  }));
  return (
    <div className={styles.rules}>
      {rules.fields.map((rule, index) => (
        <div key={rule.id} className={styles.rule}>
          <ChoiceInput
            name={`rules.${index}.field`}
            label={`Rule ${index + 1} field`}
            options={fieldOptions}
            isLabelHidden
            isRequired
          />
          <ChoiceInput
            name={`rules.${index}.condition`}
            label={`Rule ${index + 1} condition`}
            options={CONDITION_OPTIONS}
            isLabelHidden
            isRequired
          />
          <TextInput name={`rules.${index}.value`} label={`Rule ${index + 1} value`} isRequired />
          <Button
            variant="quiet"
            aria-label={`Remove rule ${index + 1}`}
            onPress={() => rules.remove(index)}
          >
            <X aria-hidden="true" />
          </Button>
        </div>
      ))}
      <div className={row.row}>
        {rules.fields.length < MAX_RULES && (
          <Button onPress={() => rules.append({ field: "", condition: "", value: "" })}>
            + Add rule
          </Button>
        )}
        <span className={styles.hint}>
          All rules must match. No rules = all messages of this type. For is one of, separate codes
          with commas.
        </span>
      </div>
    </div>
  );
}
