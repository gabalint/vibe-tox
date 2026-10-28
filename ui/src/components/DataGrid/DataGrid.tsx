import {
  type ColumnDef,
  createSortedRowModel,
  type RowData,
  rowSortingFeature,
  sortFn_alphanumeric,
  tableFeatures,
  useTable,
} from "@tanstack/react-table";
import { useVirtualizer, type VirtualItem } from "@tanstack/react-virtual";
import {
  type CSSProperties,
  type ReactNode,
  type RefObject,
  useEffect,
  useMemo,
  useRef,
} from "react";
import styles from "./DataGrid.module.css";
import { DataGridHeader } from "./DataGridHeader";
import { DataGridRow } from "./DataGridRow";
import { HEADER_ROW } from "./gridKeyboard";
import { useGridSelection } from "./gridSelection";
import { type GridBulk, SelectionBar, useListedSelection } from "./SelectionBar";
import { useGridCells, useReturnFocusToRow } from "./useGridCells";
import { type GridRowActions, useRowMenu } from "./useRowMenu";

const features = tableFeatures({
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
  sortFns: { alphanumeric: sortFn_alphanumeric },
});

/**
 * Row height, so long lists scroll without measuring each row (NFR-33). Rows of grids with a
 * wrapping column are at least this tall and are measured.
 */
const ROW_HEIGHT = 32;
/** Narrowest width of a column without a fixed `width`, in rem. */
const FLEX_COLUMN_REM = 8;
/**
 * Narrowest width of an identifier column without a fixed `width`, in rem, so near-duplicate
 * names stay readable when the grid is squeezed beside a side panel (it scrolls sideways).
 */
const IDENTIFIER_COLUMN_REM = 12;
/** Up to this many rows render at once; above it only the visible window does. */
const VIRTUALIZE_ABOVE = 100;
const OVERSCAN = 10;

export interface DataGridColumn<Row extends RowData> {
  id: string;
  header: string;
  cell: (row: Row) => ReactNode;
  /**
   * The value to sort by; columns without one are not sortable. Rows whose value is
   * `undefined` stay at the bottom in either direction.
   */
  sortValue?: (row: Row) => string | number | undefined;
  /**
   * Sorts the list by this column when it first shows, instead of the engine's order. A
   * click on the header then turns the direction around. Set on one column at most.
   */
  defaultSort?: "asc" | "desc";
  /** Fixed width in pixels; other columns share the remaining space. */
  width?: number;
  /**
   * Lets long text wrap onto more lines, for status and summary columns. Otherwise text that
   * does not fit ends in "…" and shows in full on hover.
   */
  wrap?: boolean;
  /** `end` for numbers and counts, so their digits line up. */
  align?: "end";
  /**
   * The identifier people use, such as a code or accession number, styled as a link so it
   * is visible that rows open. Not a link itself: a click on the row or Enter opens it.
   */
  identifier?: boolean;
}

interface DataGridProps<Row extends RowData> {
  /** Accessible name of the grid. */
  label: string;
  /** Keep stable between renders (module constant or `useMemo`). */
  columns: readonly DataGridColumn<Row>[];
  /**
   * Keep stable between renders; the engine's order is shown until a column is sorted,
   * unless a column has a `defaultSort`.
   */
  rows: readonly Row[];
  getRowId: (row: Row) => string;
  /**
   * Opens a row's record: a click anywhere on the row, except on a control inside it or a
   * drag that selected text, or Enter on any of its cells. `order` holds the row IDs in
   * display order, for Previous and Next. Arrow keys only move focus.
   */
  onRowAction?: (row: Row, order: readonly string[]) => void;
  /**
   * The row whose record is open: tinted, with a bar on its left edge and `aria-current`.
   * When it becomes null while focus has nowhere to be (the side panel holding focus
   * closed), focus returns to that row.
   */
  openRowId?: string | null;
  /**
   * Called with the rows' IDs in display order whenever that order changes (a sort, a find, a
   * new answer), so a side panel's Previous and Next follow the list as shown: pass
   * `useOpenRecord().followOrder`. Keep it stable between renders.
   */
  onOrderChange?: (order: readonly string[]) => void;
  /**
   * A last column with each row's actions menu, also opened by right-click and Shift+F10. It
   * stays in view at the inline end when the grid scrolls sideways.
   */
  rowActions?: GridRowActions<Row>;
  /** Bulk actions: a check column, Space to toggle, select all, and a selection bar above. */
  bulk?: GridBulk;
  /**
   * For the main grid of a list page: takes the remaining height of its flex-column parent,
   * such as `Page`, instead of at most 70% of the window.
   */
  fill?: boolean;
}

/** A sortable, keyboard-navigable and virtualized table of records that open on a click. */
export function DataGrid<Row extends RowData>(props: DataGridProps<Row>) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const grid = useGridModel(props, scrollRef);
  const { table, sortedRows, selection, select, virtualizer, shape, cells } = grid;
  return (
    <>
      {props.bulk && (
        <SelectionBar
          count={select.count}
          actions={props.bulk.actions}
          notice={grid.notice}
          onClear={select.clear}
        />
      )}
      {/* biome-ignore lint/a11y/useSemanticElements: ARIA grid on divs, see GridCell. */}
      <div
        ref={scrollRef}
        role="grid"
        aria-label={props.label}
        aria-rowcount={sortedRows.length + 1}
        aria-colcount={shape.colCount}
        aria-multiselectable={selection ? true : undefined}
        data-fill={props.fill || undefined}
        data-opens={props.onRowAction ? true : undefined}
        className={styles.grid}
      >
        <DataGridHeader
          columns={props.columns}
          layout={shape.layout}
          selectAll={selection ? select.allState : undefined}
          actionsColumn={props.rowActions !== undefined}
          sortDirection={(id) => table.getColumn(id)?.getIsSorted() ?? false}
          tabStopCol={cells.tabStop.row === HEADER_ROW ? cells.tabStop.col : undefined}
          handlers={cells.handlers}
        />
        <div className={styles.body} style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((item) => renderRow(props, grid, item))}
        </div>
      </div>
    </>
  );
}

/** The grid's rows in display order, selection, row menu, virtualizer and cell behavior. */
function useGridModel<Row extends RowData>(
  props: DataGridProps<Row>,
  scrollRef: RefObject<HTMLDivElement | null>,
) {
  const { columns, rows, getRowId, onRowAction, rowActions } = props;
  const table = useTable({
    features,
    columns: useColumnDefs(columns),
    data: rows,
    getRowId,
    initialState: { sorting: initialSorting(columns) },
  });
  const sortedRows = table.getRowModel().rows;
  const rowIds = useMemo(() => sortedRows.map((row) => row.id), [sortedRows]);
  const listed = useListedSelection(props.bulk, rowIds);
  const selection = listed.selection;
  const select = useGridSelection(selection, rowIds);
  const menu = useRowMenu(rowActions, sortedRows);
  const virtualizer = useRowVirtualizer(sortedRows.length, scrollRef);
  const shape = useGridShape(columns, selection !== undefined, rowActions !== undefined);
  const cells = useGridCells({
    size: { rowCount: sortedRows.length, colCount: shape.colCount },
    gridRef: scrollRef,
    rowHeight: ROW_HEIGHT,
    renderedRows: virtualizer.getVirtualItems().map((item) => item.index),
    selectable: selection !== undefined,
    scrollToRow: (index) => virtualizer.scrollToIndex(index),
    onHeaderAction: (col) => {
      if (selection && col === 0) return select.toggleAll();
      const id = columns[col - shape.offset]?.id;
      const column = id === undefined ? undefined : table.getColumn(id);
      if (column?.getCanSort()) column.toggleSorting();
    },
    onToggleRow: select.toggleRowAt,
    onRowAction:
      onRowAction &&
      ((index) => {
        const row = sortedRows[index];
        if (row) onRowAction(row.original, rowIds);
      }),
    onRowMenu: rowActions && menu.openAt,
  });
  useReturnFocusToRow(props.openRowId ?? null, rowIds, cells.focusRow);
  const { onOrderChange } = props;
  // Synchronizes the caller's copy of the display order, which lives outside the grid.
  useEffect(() => onOrderChange?.(rowIds), [onOrderChange, rowIds]);
  // Rows of grids with a wrapping column are measured.
  const measureRef = columns.some((column) => column.wrap) ? virtualizer.measureElement : undefined;
  const { notice } = listed;
  return {
    table,
    sortedRows,
    selection,
    select,
    menu,
    virtualizer,
    shape,
    cells,
    notice,
    measureRef,
  };
}

type GridModel<Row extends RowData> = ReturnType<typeof useGridModel<Row>>;

/** One virtual row; only per-row values and stable objects, so unchanged rows skip rendering. */
function renderRow<Row extends RowData>(
  props: DataGridProps<Row>,
  grid: GridModel<Row>,
  item: VirtualItem,
) {
  const { columns, rowActions, openRowId } = props;
  const { cells, menu } = grid;
  const row = grid.sortedRows[item.index];
  if (!row) return null;
  const hasActions = rowActions?.hasItems(row.original) ?? false;
  return (
    <DataGridRow
      key={row.id}
      row={row.original}
      rowId={row.id}
      index={item.index}
      top={item.start - ROW_HEIGHT}
      columns={columns}
      layout={grid.shape.layout}
      isSelected={grid.selection ? grid.select.isSelected(row.id) : undefined}
      isOpen={row.id === openRowId}
      tabStopCol={cells.tabStop.row === item.index ? cells.tabStop.col : undefined}
      handlers={cells.handlers}
      measureRef={grid.measureRef}
      actionsColumn={rowActions !== undefined}
      actionsLabel={rowActions && hasActions ? rowActions.label(row.original) : null}
      menuItems={menu.menu?.rowId === row.id ? menu.menu.items : null}
      onMenuChange={menu.onMenuChange}
    />
  );
}

function useColumnDefs<Row extends RowData>(columns: readonly DataGridColumn<Row>[]) {
  return useMemo(() => columns.map(toColumnDef), [columns]);
}

function toColumnDef<Row extends RowData>(
  column: DataGridColumn<Row>,
): ColumnDef<typeof features, Row> {
  const { id, header, sortValue, defaultSort } = column;
  if (sortValue === undefined) return { id, header, enableSorting: false };
  return {
    id,
    header,
    accessorFn: sortValue,
    sortFn: "alphanumeric",
    sortDescFirst: defaultSort === "desc",
    sortUndefined: "last",
  };
}

function initialSorting<Row extends RowData>(columns: readonly DataGridColumn<Row>[]) {
  const column = columns.find((candidate) => candidate.defaultSort !== undefined);
  return column ? [{ id: column.id, desc: column.defaultSort === "desc" }] : [];
}

function useRowVirtualizer(count: number, scrollRef: RefObject<HTMLDivElement | null>) {
  return useVirtualizer({
    count,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: count <= VIRTUALIZE_ABOVE ? VIRTUALIZE_ABOVE : OVERSCAN,
    // The body starts below the sticky header row, which also covers the top of the view.
    scrollMargin: ROW_HEIGHT,
    scrollPaddingStart: ROW_HEIGHT,
  });
}

/** The column count, the check column's offset and the row layout, stable across renders. */
function useGridShape<Row extends RowData>(
  columns: readonly DataGridColumn<Row>[],
  selectable: boolean,
  hasActions: boolean,
) {
  const layout = useMemo(
    () => rowLayout(columns, selectable, hasActions),
    [columns, selectable, hasActions],
  );
  const offset = selectable ? 1 : 0;
  return { layout, offset, colCount: columns.length + offset + (hasActions ? 1 : 0) };
}

/**
 * The column template shared by the header and every row, and their minimum width: the sum
 * of the columns' minimums, so the sticky header and row backgrounds span a grid wider than
 * its container when it scrolls sideways.
 */
function rowLayout<Row extends RowData>(
  columns: readonly DataGridColumn<Row>[],
  selectable: boolean,
  hasActions: boolean,
): CSSProperties {
  const minRem = (column: DataGridColumn<Row>) =>
    column.width !== undefined ? 0 : column.identifier ? IDENTIFIER_COLUMN_REM : FLEX_COLUMN_REM;
  const widths = columns.map((column) =>
    column.width === undefined ? `minmax(${minRem(column)}rem, 1fr)` : `${column.width}px`,
  );
  const fixedPx = columns.reduce((sum, column) => sum + (column.width ?? 0), 0);
  const flexRem = columns.reduce((sum, column) => sum + minRem(column), 0);
  const check = selectable ? "var(--control-height)" : "0px";
  const actions = hasActions ? "calc(var(--control-height) + var(--space-2))" : "0px";
  return {
    gridTemplateColumns: [
      ...(selectable ? [check] : []),
      ...widths,
      ...(hasActions ? [actions] : []),
    ].join(" "),
    minWidth: `max(100%, calc(${check} + ${actions} + ${fixedPx}px + ${flexRem}rem))`,
  };
}
