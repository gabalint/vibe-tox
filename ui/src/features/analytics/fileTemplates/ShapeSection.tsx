import { useFormContext, useWatch } from "react-hook-form";
import { CheckList, ChoiceInput } from "@/components/FormFields";
import { Section } from "@/components/Section";
import { ChoiceField } from "./ChoiceField";
import { type ColumnOption, fieldNames, headerGroups } from "./header";
import { SHAPES } from "./templateLabels";
import { type TemplateValues, trims } from "./templateValues";

interface ShapeSectionProps {
  header: string[][];
  /** The header's columns by position, with example values. */
  columns: readonly ColumnOption[];
}

/**
 * Step 3: how the file lays out samples and analytes (F4). Long files name the analyte in a
 * column; wide files hold one analyte per column, or per column group under a two-row header.
 */
export function ShapeSection({ header, columns }: ShapeSectionProps) {
  const shape = useWatch<TemplateValues, "layout.shape">({ name: "layout.shape" });
  return (
    <Section title="3. Shape">
      <ChoiceInput name="layout.shape" label="Shape" options={SHAPES} isRequired />
      {shape === "long" && (
        <>
          <ChoiceField
            name="layout.analyteColumn"
            label="Analyte column"
            options={columns}
            isRequired
          />
          <ChoiceField
            name="layout.valueColumn"
            label="Value column"
            options={columns}
            isRequired
          />
        </>
      )}
      {shape === "wide" && header.length === 2 && <GroupChoice header={header} />}
      {shape === "wide" && header.length === 1 && (
        <CheckList
          name="layout.analyteColumns"
          legend="Analyte columns"
          options={columns.map(({ id, label }) => ({ id, label }))}
          requiredMessage="Tick at least one analyte column."
        />
      )}
    </Section>
  );
}

/**
 * A two-row header: the value field, then one value column per group that has it, named by
 * the group label. Every such group starts kept; drop groups such as internal standards.
 */
function GroupChoice({ header }: { header: string[][] }) {
  const { setValue } = useFormContext<TemplateValues>();
  const field = useWatch<TemplateValues, "layout.valueField">({ name: "layout.valueField" });
  const trimFields = useWatch<TemplateValues, "layout.trimFields">({ name: "layout.trimFields" });
  const trim = trims({ trimFields });
  const groups = field === null ? [] : headerGroups(header, field, trim);
  const withField = groups.flatMap(({ label, column }) =>
    column === null ? [] : [{ id: String(column), label, details: [`Column ${column + 1}`] }],
  );
  const without = groups.filter(({ column }) => column === null).map(({ label }) => label);
  const keepAll = (value: string | null) =>
    setValue(
      "layout.analyteColumns",
      value === null
        ? []
        : headerGroups(header, value, trim).flatMap(({ column }) =>
            column === null ? [] : [String(column)],
          ),
      { shouldDirty: true },
    );
  return (
    <>
      <ChoiceField
        name="layout.valueField"
        label="Value field"
        description="The field of each analyte group that holds the reported value, such as Final Conc."
        options={fieldNames(header, trim).map((name) => ({ id: name, label: name }))}
        isRequired
        onChange={keepAll}
      />
      {field !== null && (
        <CheckList
          name="layout.analyteColumns"
          legend="Analyte groups read"
          options={withField}
          requiredMessage="Keep at least one analyte group."
        />
      )}
      {without.length > 0 && <p>Groups without that field give no rows: {without.join(", ")}.</p>}
    </>
  );
}
