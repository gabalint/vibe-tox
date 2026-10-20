import { useMemo, useState } from "react";
import { FindField, findRows, NoResults } from "@/components/FindField";
import { Toolbar } from "@/components/Toolbar";
import type { StandardCatalogLoaded } from "@/generated/StandardCatalogLoaded";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { LoadSummary } from "./LoadSummary";
import { MethodList } from "./MethodList";
import { OrderableList } from "./OrderableList";
import { deriveSelection, NO_CHOICES } from "./selection";
import styles from "./starter.module.css";

interface CatalogChooserProps {
  tree: StandardCatalogTree;
  onLoaded: (result: StandardCatalogLoaded) => void;
  onReload: () => void;
}

/**
 * The selection view: tests and panels beside methods, one Find for both, then the summary.
 * Selected items stay listed whatever the find, so nothing chosen is hidden. The file is
 * bounded (tens of items), so the UI filters it. Leaving with a selection asks first.
 */
export function CatalogChooser({ tree, onLoaded, onReload }: CatalogChooserProps) {
  const [choices, setChoices] = useState(NO_CHOICES);
  // Local, not `?q=`: the unsaved-selection guard would stop every change of the URL.
  const [query, setQuery] = useState("");
  const selection = useMemo(() => deriveSelection(tree, choices), [tree, choices]);
  const orderables = useMemo(() => {
    const found = new Set(findRows(tree.orderables, query, (o) => [o.code, o.name]));
    return tree.orderables.filter((o) => found.has(o) || choices.orderables.has(o.code));
  }, [tree, query, choices]);
  const methods = useMemo(() => {
    const found = new Set(
      findRows(tree.methods, query, (m) => [
        m.code,
        m.name,
        ...m.analytes.map((a) => a.analyteCode),
      ]),
    );
    return tree.methods.filter((m) => found.has(m) || selection.methods.has(m.code));
  }, [tree, query, selection]);
  return (
    <>
      <UnsavedChangesGuard when={selection.methods.size > 0} />
      <Toolbar variant="collection">
        <FindField
          label="Find tests, panels and methods"
          placeholder="Code, name or analyte"
          value={query}
          onChange={setQuery}
        />
      </Toolbar>
      {orderables.length === 0 && methods.length === 0 ? (
        <NoResults noun="tests, panels or methods" query={query} onClear={() => setQuery("")} />
      ) : (
        <div className={styles.columns}>
          <OrderableList tree={tree} shown={orderables} choices={choices} onChange={setChoices} />
          <MethodList
            tree={tree}
            shown={methods}
            choices={choices}
            selection={selection}
            onChange={setChoices}
          />
        </div>
      )}
      <LoadSummary
        tree={tree}
        choices={choices}
        selection={selection}
        onLoaded={onLoaded}
        onReload={onReload}
      />
    </>
  );
}
