import type { GridRowActions } from "@/components/DataGrid";
import type { ActionItem } from "@/components/RowActions";

/**
 * One action a record view may offer, named by the engine command that appears in the
 * record's `allowedActions`. A screen lists its actions once, in the order shown.
 */
export interface OfferedAction<Id extends string> {
  id: Id;
  label: string;
  /** Red text, for destructive actions. */
  variant?: "danger";
}

/**
 * The record's actions that the engine allows (`allowed` is its `allowedActions`), in the
 * order of `actions`, as items for `MoreActions` or a row menu. `onAction` runs the chosen
 * one, usually by opening its dialog. Presentation only: the engine decides each command.
 */
export function allowedActionItems<Id extends string>(
  actions: readonly OfferedAction<Id>[],
  allowed: readonly string[],
  onAction: (id: Id) => void,
): ActionItem[] {
  return actions
    .filter(({ id }) => allowed.includes(id))
    .map(({ id, label, variant }) => ({
      id,
      label,
      ...(variant && { variant }),
      onAction: () => onAction(id),
    }));
}

interface RowActionsOf<Row, Id extends string> {
  /** The row's identifier, as people know it: names the "⋯" button. */
  label: (row: Row) => string;
  /** The row's `allowedActions`. */
  allowed: (row: Row) => readonly string[];
  /** Runs the chosen action on the row. */
  onAction: (id: Id, row: Row) => void;
}

/**
 * A `DataGrid`'s `rowActions` from the same `actions` the record view shows: the menu holds
 * the allowed ones in that order, and `hasItems` only checks the row's `allowedActions`, so
 * it stays cheap while rendering.
 */
export function allowedRowActions<Row, Id extends string>(
  actions: readonly OfferedAction<Id>[],
  { label, allowed, onAction }: RowActionsOf<Row, Id>,
): GridRowActions<Row> {
  return {
    label,
    hasItems: (row) => actions.some(({ id }) => allowed(row).includes(id)),
    items: (row) => allowedActionItems(actions, allowed(row), (id) => onAction(id, row)),
  };
}
