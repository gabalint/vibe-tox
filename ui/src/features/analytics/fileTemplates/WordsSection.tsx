import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import { Select } from "@/components/Select";
import type { PositionRole } from "@/generated/PositionRole";
import type { ValueWordMeaning } from "@/generated/ValueWordMeaning";
import { choiceOf } from "@/lib/choices";
import layout from "@/styles/layout.module.css";
import { POSITION_ROLES } from "../labels";
import styles from "./fileTemplates.module.css";
import { type Mappings, mapSampleType, mapValueWord } from "./mappings";
import { UNMAPPED, VALUE_MEANINGS } from "./templateLabels";
import type { Counted, Found } from "./templateValues";

const NOT_MAPPED = { id: UNMAPPED, label: "Not mapped" };
const ROLE_OPTIONS = [NOT_MAPPED, ...POSITION_ROLES];
const MEANING_OPTIONS = [NOT_MAPPED, ...VALUE_MEANINGS];

interface WordsSectionProps {
  found: Found;
  /** Sample-type words are read only from a sample type column (F9). */
  hasTypeColumn: boolean;
  mappings: Mappings;
  onChange: (mappings: Mappings) => void;
}

/**
 * Step 6: the words found in the sample (F7, F9; Q-40). A sample-type word says whether a row
 * is patient material or QC; the run position still decides. A value word such as N/F is
 * kept as received, and the result is recorded from the method version's limit with this
 * rule. Unmapped words, and every empty value, are quarantined.
 */
export function WordsSection({ found, hasTypeColumn, mappings, onChange }: WordsSectionProps) {
  return (
    <Section title="6. Words">
      {hasTypeColumn && (
        <WordList
          heading="Sample types"
          field="sample-type"
          emptyText="No sample type words were found."
          words={found.typeWords}
          options={ROLE_OPTIONS}
          value={(word) => mappings.sampleTypes.get(word) ?? null}
          onChange={(word, id) =>
            onChange(mapSampleType(mappings, word, choiceOf<PositionRole>(POSITION_ROLES, id)))
          }
        />
      )}
      <WordList
        heading="Value words"
        field="value-word"
        emptyText="Every value in the sample is a positive number or empty."
        words={found.valueTexts}
        options={MEANING_OPTIONS}
        value={(word) => mappings.valueWords.get(word) ?? null}
        onChange={(word, id) =>
          onChange(mapValueWord(mappings, word, choiceOf<ValueWordMeaning>(VALUE_MEANINGS, id)))
        }
      />
      <Facts items={[{ label: "Empty values", value: String(found.empty) }]} />
      <p>Empty cells are always quarantined.</p>
    </Section>
  );
}

interface WordListProps {
  heading: string;
  /** The fields' name prefix. */
  field: string;
  emptyText: string;
  words: readonly Counted[];
  options: readonly { id: string; label: string }[];
  value: (word: string) => string | null;
  onChange: (word: string, id: string | null) => void;
}

/** Each distinct word, exactly as written, with its row count and its meaning. */
function WordList({ heading, field, emptyText, words, options, value, onChange }: WordListProps) {
  return (
    <>
      <h3 className={layout.subheading}>{heading}</h3>
      {words.length === 0 ? (
        <p>{emptyText}</p>
      ) : (
        <div className={styles.mappings}>
          {words.map(({ text, count }, index) => (
            <Select
              key={text}
              label={text}
              name={`${field}-${index}`}
              description={`${count} ${count === 1 ? "row" : "rows"}`}
              placeholder="Not mapped"
              options={options}
              value={value(text) ?? UNMAPPED}
              onChange={(id) => onChange(text, id)}
            />
          ))}
        </div>
      )}
    </>
  );
}
