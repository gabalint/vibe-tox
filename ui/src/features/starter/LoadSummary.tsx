import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { LoadStandardCatalog } from "@/generated/LoadStandardCatalog";
import type { StandardCatalogLoaded } from "@/generated/StandardCatalogLoaded";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import { CommandRefusal } from "@/lib/CommandRefusal";
import { useCommandMutation } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { LOAD_INVALIDATES, LOAD_STANDARD_CATALOG } from "./contract";
import { loadRefusal } from "./refusals";
import { type Choices, type Counts, type Selection, toLoadInput } from "./selection";
import styles from "./starter.module.css";

interface LoadSummaryProps {
  tree: StandardCatalogTree;
  choices: Choices;
  selection: Selection;
  onLoaded: (result: StandardCatalogLoaded) => void;
  /** Reads the list again, after the engine says it changed. */
  onReload: () => void;
}

function countFacts(counts: Counts) {
  return [
    { label: "Specimen types", value: String(counts.specimenTypes) },
    { label: "Analytes", value: String(counts.analytes) },
    { label: "Methods", value: String(counts.methods) },
    { label: "Tests", value: String(counts.tests) },
    { label: "Panels", value: String(counts.panels) },
    { label: "Already in your catalog, not loaded again", value: String(counts.skipped) },
  ];
}

/** Why Load is not available yet, so a disabled button always has its reason. */
function waitingFor(selection: Selection, acknowledged: boolean): string | null {
  if (selection.methods.size === 0) return "Choose at least one test, panel or method.";
  if (selection.blockers.length > 0) return "Remove the items that cannot be loaded.";
  if (!acknowledged) return "Tick “I have read this” to load.";
  return null;
}

/**
 * What the selection creates, per kind, and the not-for-production disclaimer, which must be
 * acknowledged before Load. Loading creates drafts only; codes are permanent.
 */
export function LoadSummary({ tree, choices, selection, onLoaded, onReload }: LoadSummaryProps) {
  const [acknowledged, setAcknowledged] = useState(false);
  const load = useCommandMutation<StandardCatalogLoaded, { input: LoadStandardCatalog }>(
    LOAD_STANDARD_CATALOG,
    { invalidates: LOAD_INVALIDATES },
  );
  const waiting = waitingFor(selection, acknowledged);
  const run = () =>
    load.mutate({ input: toLoadInput(tree, choices, acknowledged) }, { onSuccess: onLoaded });
  return (
    <Section title="Summary">
      <p>To be created as drafts:</p>
      <Facts layout="compact" items={countFacts(selection.counts)} />
      <p>Nothing loaded can be removed later: catalog codes are permanent.</p>
      {selection.blockers.length > 0 && (
        <Alert tone="warning" title="These items cannot be loaded">
          <ul className={layout.list}>
            {selection.blockers.map((blocker) => (
              <li key={blocker.code}>
                {blocker.name} <span className={layout.code}>{blocker.code}</span>: {blocker.reason}
              </li>
            ))}
          </ul>
          <p>Remove them from their methods, or clear the methods that need them.</p>
        </Alert>
      )}
      <Alert tone="warning" title="Starter content">
        {tree.info.disclaimer}
      </Alert>
      <Checkbox isSelected={acknowledged} onChange={setAcknowledged}>
        I have read this
      </Checkbox>
      <div className={styles.actions}>
        <Button
          variant="primary"
          isDisabled={waiting !== null}
          isPending={load.isPending}
          onPress={run}
        >
          Load Standard Catalog
        </Button>
        {waiting && <span className={styles.note}>{waiting}</span>}
      </div>
      {load.error && (
        <>
          <CommandRefusal title="Not loaded" error={load.error} lead={loadRefusal(load.error)} />
          {load.error.code === "conflict" && (
            <div>
              <Button onPress={onReload}>Reload the list</Button>
            </div>
          )}
        </>
      )}
    </Section>
  );
}
