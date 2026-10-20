import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Section } from "@/components/Section";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import type { StandardMethodAnalyteItem } from "@/generated/StandardMethodAnalyteItem";
import type { StandardMethodItem } from "@/generated/StandardMethodItem";
import { ItemCheck } from "./ItemCheck";
import {
  alsoRemovedFrom,
  type Choices,
  chooseMethod,
  keepAnalyte,
  nameOf,
  omissionKey,
  removalBlock,
  type Selection,
  unavailableText,
} from "./selection";
import styles from "./starter.module.css";

interface MethodListProps {
  tree: StandardCatalogTree;
  /** The methods the find leaves, in file order. */
  shown: readonly StandardMethodItem[];
  choices: Choices;
  selection: Selection;
  onChange: (choices: Choices) => void;
}

/** Why an analyte could not be removed, shown under its method. */
interface Notice {
  methodCode: string;
  text: string;
}

/**
 * "Methods": choosing one selects its specimen type and analytes. A method a chosen test or
 * panel needs is checked and locked. An analyte of a selected method can be removed, except its
 * last one; the tests and panels using it lose it too.
 */
export function MethodList({ tree, shown, choices, selection, onChange }: MethodListProps) {
  const [notice, setNotice] = useState<Notice | null>(null);
  return (
    <Section title={`Methods (${shown.length} of ${tree.methods.length})`}>
      <ul className={styles.items}>
        {shown.map((method) => {
          const neededBy = selection.methodNeeders.get(method.code);
          const selected = selection.methods.has(method.code);
          const keep = (analyteCode: string, on: boolean) => {
            const block = on ? null : removalBlock(tree, choices, method, analyteCode);
            setNotice(block === null ? null : { methodCode: method.code, text: block });
            if (block === null) onChange(keepAnalyte(choices, method.code, analyteCode, on));
          };
          return (
            <li key={method.code}>
              <ItemCheck
                name={method.name}
                code={method.code}
                note={
                  unavailableText(method) ?? (neededBy ? `Needed by ${neededBy.join(", ")}` : null)
                }
                isSelected={selected}
                isDisabled={method.state !== "new" || neededBy !== undefined}
                onChange={(on) => onChange(chooseMethod(tree, choices, method.code, on))}
              />
              <details className={styles.details}>
                <summary>Specimen type and analytes ({method.analytes.length})</summary>
                <MethodContent
                  tree={tree}
                  method={method}
                  selected={selected}
                  choices={choices}
                  onKeep={keep}
                />
              </details>
              {notice?.methodCode === method.code && <Alert tone="warning">{notice.text}</Alert>}
            </li>
          );
        })}
      </ul>
    </Section>
  );
}

interface MethodContentProps {
  tree: StandardCatalogTree;
  method: StandardMethodItem;
  selected: boolean;
  choices: Choices;
  onKeep: (analyteCode: string, keep: boolean) => void;
}

function MethodContent({ tree, method, selected, choices, onKeep }: MethodContentProps) {
  const type = tree.specimenTypes.find((each) => each.code === method.specimenTypeCode);
  const typeNote = type && unavailableText(type);
  return (
    <ul className={styles.subItems}>
      <li>
        <ItemCheck
          name={`Specimen type: ${type?.name ?? method.specimenTypeCode}`}
          code={method.specimenTypeCode}
          note={typeNote ?? (selected ? `Needed by ${method.name}` : null)}
          isSelected={selected}
          isDisabled
          onChange={() => undefined}
        />
      </li>
      {method.analytes.map((analyte) => {
        const removed = choices.omitted.has(omissionKey(method.code, analyte.analyteCode));
        const item = tree.analytes.find((each) => each.code === analyte.analyteCode);
        return (
          <li key={analyte.analyteCode}>
            <ItemCheck
              name={nameOf(tree.analytes, analyte.analyteCode)}
              code={analyte.analyteCode}
              note={item ? unavailableText(item) : null}
              isSelected={selected && !removed}
              isDisabled={!selected}
              removed={selected && removed}
              onChange={(keep) => onKeep(analyte.analyteCode, keep)}
            />
            <p className={styles.note}>
              <Cutoff analyte={analyte} />
              {selected && removed && (
                <RemovedFrom
                  names={alsoRemovedFrom(tree, choices, method.code, analyte.analyteCode)}
                />
              )}
            </p>
          </li>
        );
      })}
    </ul>
  );
}

/** The unit and the cutoff with its source label, exactly as the file gives them. */
function Cutoff({ analyte }: { analyte: StandardMethodAnalyteItem }) {
  const unit = analyte.unit ?? "no unit";
  if (analyte.cutoff === null) {
    return <>Unit {unit}. Cutoff blank: laboratory to supply.</>;
  }
  const value = analyte.unit === null ? analyte.cutoff : `${analyte.cutoff} ${analyte.unit}`;
  return (
    <>
      Unit {unit}. Cutoff <span className={styles.value}>{value}</span>
      {analyte.cutoffLabel && ` (${analyte.cutoffLabel})`}.
    </>
  );
}

function RemovedFrom({ names }: { names: readonly string[] }) {
  const also = names.length > 0 ? ` Also removed from ${names.join(", ")}.` : "";
  return <> Removed from this method.{also}</>;
}
