import { useWatch } from "react-hook-form";
import { Facts } from "@/components/Facts";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { ChoiceField } from "./ChoiceField";
import type { ColumnOption } from "./header";
import { END_OF_DATA, NOT_IN_FILE } from "./templateLabels";
import type { TemplateValues } from "./templateValues";

const OPTIONAL_ROLES = [
  {
    role: "sampleType",
    label: "Sample type column",
    description: "Its words are mapped below to patient, control, calibrator or blank.",
  },
  { role: "flags", label: "Flags column" },
  { role: "operator", label: "Operator column" },
  { role: "acquiredAt", label: "Acquisition time column" },
  {
    role: "dilution",
    label: "Dilution factor column",
    description: "Each value must equal the dilution recorded in the LIS for that test.",
  },
] as const;

/** The columns of an optional role, after the choice that the file has none. */
function withNone(columns: readonly ColumnOption[]): ColumnOption[] {
  return [{ id: NOT_IN_FILE, label: "Not in the file" }, ...columns];
}

/**
 * Step 4: which column holds what, by position (F5), and how values are read. Every other
 * column stays only in the received file.
 */
export function ColumnsSection({ columns }: { columns: readonly ColumnOption[] }) {
  const optional = withNone(columns);
  return (
    <Section title="4. Columns">
      <ChoiceField
        name="layout.columns.sampleLabel"
        label="Sample label column"
        description="The container label for patient samples, or the lot's sample label for controls and calibrators."
        options={columns}
        isRequired
      />
      {OPTIONAL_ROLES.map(({ role, ...field }) => (
        <ChoiceField key={role} name={`layout.columns.${role}`} {...field} options={optional} />
      ))}
      <ChoiceInput name="layout.endOfData" label="End of data" options={END_OF_DATA} isRequired />
    </Section>
  );
}

/**
 * Step 7: where the values' units come from (F8): a units column, whose text must equal the
 * method's unit, or one unit declared for every value. Without either, rows whose method has
 * a unit are quarantined.
 */
export function UnitsSection({ columns }: { columns: readonly ColumnOption[] }) {
  const unitsColumn = useWatch<TemplateValues, "layout.columns.units">({
    name: "layout.columns.units",
  });
  const hasColumn = unitsColumn !== null && unitsColumn !== NOT_IN_FILE;
  return (
    <Section title="7. Units">
      <ChoiceField name="layout.columns.units" label="Units column" options={withNone(columns)} />
      {hasColumn ? (
        <Facts
          items={[{ label: "Declared unit", value: "None: the units column gives each unit" }]}
        />
      ) : (
        <TextInput
          name="layout.declaredUnit"
          label="Declared unit"
          description="The unit of every value, exactly as the method writes it, such as ng/mL. Leave empty for methods without a unit."
        />
      )}
    </Section>
  );
}
