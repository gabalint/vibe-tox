import { useMemo } from "react";

/** Controlled row selection by row ID. */
export interface GridSelection {
  /** The selected rows' IDs. */
  selectedIds: ReadonlySet<string>;
  /** Called with the new set whenever the selection changes. */
  onChange: (selectedIds: ReadonlySet<string>) => void;
}

type MarkState = "checked" | "mixed" | "unchecked";

/** What the grid does with the selection. */
interface SelectActions {
  allState: MarkState;
  /** Selected rows among those listed. */
  count: number;
  isSelected: (id: string) => boolean;
  toggleRowAt: (index: number) => void;
  toggleAll: () => void;
  clear: () => void;
}

const NONE: ReadonlySet<string> = new Set();
const noop = () => undefined;

/** A grid without bulk actions: nothing is selected and nothing can be. */
const INERT: SelectActions = {
  allState: "unchecked",
  count: 0,
  isSelected: () => false,
  toggleRowAt: noop,
  toggleAll: noop,
  clear: noop,
};

/**
 * Selection actions over the rows in display order (by index); inert without `selection`.
 * The count over the listed rows is worked out only when the selection or the rows change,
 * not on every render of a 10,000-row grid (NFR-33).
 */
export function useGridSelection(
  selection: GridSelection | undefined,
  rowIds: readonly string[],
): SelectActions {
  const selectedIds = selection?.selectedIds ?? NONE;
  const count = useMemo(() => {
    if (selectedIds.size === 0) return 0;
    const listed = new Set(rowIds);
    return [...selectedIds].filter((id) => listed.has(id)).length;
  }, [selectedIds, rowIds]);
  if (!selection) return INERT;
  const allState = selectAllState(count, rowIds.length);

  function toggleRowAt(index: number) {
    const id = rowIds[index];
    if (!selection || id === undefined) return;
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selection.onChange(next);
  }

  return {
    allState,
    count,
    isSelected: (id: string) => selectedIds.has(id),
    toggleRowAt,
    toggleAll: () => selection.onChange(allState === "checked" ? new Set() : new Set(rowIds)),
    clear: () => selection.onChange(new Set()),
  };
}

function selectAllState(selected: number, total: number): MarkState {
  if (selected === 0) return "unchecked";
  return selected === total ? "checked" : "mixed";
}
