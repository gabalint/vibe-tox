import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { FileLayout } from "@/generated/FileLayout";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { labelOf } from "@/lib/choices";
import layoutStyles from "@/styles/layout.module.css";
import { POSITION_ROLES } from "../labels";
import { useAnalyteName } from "../useAnalyteName";
import { columnOptions } from "./header";
import {
  DELIMITERS,
  ENCODINGS,
  END_OF_DATA,
  SHAPES,
  TRIM_FIELDS,
  VALUE_MEANINGS,
} from "./templateLabels";

/** A list of texts, or `none` when empty. */
function List({ items, none }: { items: readonly string[]; none: string }) {
  if (items.length === 0) return none;
  return (
    <ul className={layoutStyles.list}>
      {items.map((item) => (
        <li key={item}>{item}</li>
      ))}
    </ul>
  );
}

function columnFacts(layout: FileLayout) {
  const names = columnOptions(layout.header, [], layout.trimFields).map(({ label }) => label);
  const name = (column: number | null) =>
    column === null ? "Not in the file" : (names[column] ?? `${column + 1}`);
  const { columns, shape } = layout;
  const shapeFacts =
    shape.kind === "long"
      ? [
          { label: "Analyte column", value: name(shape.analyteColumn) },
          { label: "Value column", value: name(shape.valueColumn) },
        ]
      : [
          {
            label: "Analyte value columns",
            value: (
              <List
                items={shape.valueColumns.map(
                  (value) => `${value.analyteName}: ${name(value.column)}`,
                )}
                none="None"
              />
            ),
          },
        ];
  return [
    { label: "Shape", value: labelOf(SHAPES, shape.kind) },
    ...shapeFacts,
    { label: "Sample label column", value: name(columns.sampleLabel) },
    { label: "Sample type column", value: name(columns.sampleType) },
    { label: "Flags column", value: name(columns.flags) },
    { label: "Operator column", value: name(columns.operator) },
    { label: "Acquisition time column", value: name(columns.acquiredAt) },
    { label: "Dilution factor column", value: name(columns.dilution) },
    { label: "Units column", value: name(columns.units) },
    { label: "Declared unit", value: layout.declaredUnit ?? "None" },
  ];
}

interface TemplateFactsProps {
  source: SourceRegistrationView;
  layout: FileLayout;
}

/**
 * Everything a file template says, for the person approving it (brief §4): how the text is
 * read, the exact header, which column holds what, and every mapped name and word.
 */
export function TemplateFacts({ source, layout }: TemplateFactsProps) {
  const analyteName = useAnalyteName();
  return (
    <>
      <Section title="Text and header">
        <Facts
          items={[
            {
              label: "Reports final concentrations",
              value: source.reportsFinalConcentration ? "Yes" : "No",
            },
            {
              label: "Encoding without a byte-order mark",
              value: labelOf(ENCODINGS, layout.encoding),
            },
            { label: "Delimiter", value: labelOf(DELIMITERS, layout.delimiter) },
            { label: "Spaces", value: labelOf(TRIM_FIELDS, layout.trimFields ? "yes" : "no") },
            { label: "End of data", value: labelOf(END_OF_DATA, layout.endOfData) },
            {
              label: "Header lines",
              value: <List items={layout.header.map((row) => row.join(" | "))} none="None" />,
            },
          ]}
        />
      </Section>
      <Section title="Columns">
        <Facts items={columnFacts(layout)} />
      </Section>
      <Section title="Analytes and words">
        <Facts
          items={[
            {
              label: "Analytes",
              value: (
                <List
                  items={source.testCodes.map(
                    (code) => `${code.sourceTestCode} → ${analyteName(code.analyteId)}`,
                  )}
                  none="None mapped"
                />
              ),
            },
            { label: "Not results", value: <List items={layout.notResults} none="None" /> },
            {
              label: "Sample types",
              value: (
                <List
                  items={layout.sampleTypes.map(
                    ({ word, role }) => `${word} → ${labelOf(POSITION_ROLES, role)}`,
                  )}
                  none="None mapped"
                />
              ),
            },
            {
              label: "Value words",
              value: (
                <List
                  items={layout.valueWords.map(
                    ({ word, meaning }) => `${word} → ${labelOf(VALUE_MEANINGS, meaning)}`,
                  )}
                  none="None mapped"
                />
              ),
            },
            { label: "Empty values", value: "Always quarantined" },
          ]}
        />
      </Section>
    </>
  );
}
