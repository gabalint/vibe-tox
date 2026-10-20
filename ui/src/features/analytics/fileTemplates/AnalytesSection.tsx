import { Checkbox } from "@/components/Checkbox";
import { Inset } from "@/components/Inset";
import { QueryContent } from "@/components/QueryContent";
import { type RecordOption, RecordPicker } from "@/components/RecordPicker";
import { Section } from "@/components/Section";
import type { AnalyteView } from "@/generated/AnalyteView";
import { useCommandQuery } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { LIST_ANALYTES } from "../contract";
import styles from "./fileTemplates.module.css";
import { type Mappings, mapAnalyte, markNotResult } from "./mappings";

interface AnalytesSectionProps {
  /** The names to map, exactly as the file writes them. */
  names: readonly string[];
  /** Long files may count a name as not a result (F10). */
  isLong: boolean;
  mappings: Mappings;
  onChange: (mappings: Mappings) => void;
}

/**
 * Step 5: each analyte name found (long) or each analyte column (wide) mapped to a catalog
 * analyte (REQ-ANA-003). In a long file a name may instead be counted as not a result, such
 * as an internal standard. Rows of a name left unmapped are quarantined (F10).
 */
export function AnalytesSection({ names, isLong, mappings, onChange }: AnalytesSectionProps) {
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  return (
    <Section title="5. Analytes">
      <p>
        {isLong
          ? `${names.length} analyte names found in the sample. Rows of a name left unmapped are quarantined.`
          : `${names.length} analyte columns. Map each one.`}
      </p>
      <QueryContent query={analytes} what="analytes">
        {(catalog) => {
          const options: RecordOption[] = catalog.map((analyte) => ({
            id: analyte.id,
            label: analyte.name,
            description: analyte.code,
          }));
          return (
            <div className={styles.mappings}>
              {names.map((name, index) => (
                <AnalyteMapping
                  key={name}
                  name={name}
                  field={`analyte-${index}`}
                  isLong={isLong}
                  options={options}
                  mappings={mappings}
                  onChange={onChange}
                />
              ))}
            </div>
          );
        }}
      </QueryContent>
    </Section>
  );
}

interface AnalyteMappingProps extends Omit<AnalytesSectionProps, "names"> {
  name: string;
  field: string;
  options: readonly RecordOption[];
}

function AnalyteMapping({ name, field, isLong, options, mappings, onChange }: AnalyteMappingProps) {
  const notResult = isLong && mappings.notResults.has(name);
  return (
    <Inset as="fieldset">
      <legend className={layout.code}>{name}</legend>
      <RecordPicker
        label="Catalog analyte"
        name={field}
        options={options}
        value={mappings.analytes.get(name) ?? null}
        onChange={(analyteId) => onChange(mapAnalyte(mappings, name, analyteId))}
        isDisabled={notResult}
        isRequired={!isLong}
      />
      {isLong && (
        <Checkbox
          isSelected={notResult}
          onChange={(isSelected) => onChange(markNotResult(mappings, name, isSelected))}
        >
          Not a result
        </Checkbox>
      )}
    </Inset>
  );
}
