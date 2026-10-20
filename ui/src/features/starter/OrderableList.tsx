import { Section } from "@/components/Section";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import type { StandardComponentItem } from "@/generated/StandardComponentItem";
import type { StandardOrderableItem } from "@/generated/StandardOrderableItem";
import layout from "@/styles/layout.module.css";
import { ItemCheck } from "./ItemCheck";
import {
  type Choices,
  chooseOrderable,
  componentAnalytes,
  nameOf,
  unavailableText,
} from "./selection";
import styles from "./starter.module.css";

interface OrderableListProps {
  tree: StandardCatalogTree;
  /** The tests and panels the find leaves, in file order. */
  shown: readonly StandardOrderableItem[];
  choices: Choices;
  onChange: (choices: Choices) => void;
}

/** "Tests and panels": choosing one selects its methods; each expands to its components. */
export function OrderableList({ tree, shown, choices, onChange }: OrderableListProps) {
  return (
    <Section title={`Tests and panels (${shown.length} of ${tree.orderables.length})`}>
      <ul className={styles.items}>
        {shown.map((orderable) => (
          <li key={orderable.code}>
            <ItemCheck
              name={orderable.name}
              code={orderable.code}
              note={unavailableText(orderable)}
              isSelected={choices.orderables.has(orderable.code)}
              isDisabled={orderable.state !== "new"}
              onChange={(on) => onChange(chooseOrderable(tree, choices, orderable.code, on))}
            />
            <details className={styles.details}>
              <summary>
                {orderable.kind === "panel" ? "Panel" : "Test"}: components (
                {orderable.components.length})
              </summary>
              <ul className={styles.subItems}>
                {orderable.components.map((component) => (
                  <ComponentItem
                    key={component.methodCode}
                    tree={tree}
                    component={component}
                    choices={choices}
                  />
                ))}
              </ul>
            </details>
          </li>
        ))}
      </ul>
    </Section>
  );
}

const PURPOSES = { analytical: "analytical", validity: "specimen validity" } as const;

function ComponentItem({
  tree,
  component,
  choices,
}: {
  tree: StandardCatalogTree;
  component: StandardComponentItem;
  choices: Choices;
}) {
  const method = nameOf(tree.methods, component.methodCode);
  return (
    <li>
      {method} <span className={layout.code}>{component.methodCode}</span>,{" "}
      {PURPOSES[component.purpose]}
      <ul className={styles.subItems}>
        {componentAnalytes(component, choices).map(({ code, removed }) => (
          <li key={code}>
            <span className={removed ? styles.removed : undefined}>
              {nameOf(tree.analytes, code)} <span className={layout.code}>{code}</span>
            </span>
            {removed && <span className={styles.note}> Removed from {method}</span>}
          </li>
        ))}
      </ul>
    </li>
  );
}
