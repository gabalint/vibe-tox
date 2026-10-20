import type { UseQueryResult } from "@tanstack/react-query";
import { type ReactNode, useMemo } from "react";
import { useLocation, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn, type GridRowActions } from "@/components/DataGrid";
import { DetailLayout } from "@/components/DetailPanel";
import { FindCount, FindField, findRows, NoResults } from "@/components/FindField";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Toolbar } from "@/components/Toolbar";
import type { CommandError } from "@/generated/CommandError";
import { useHasCapability } from "@/lib/commands";
import { useFindParam } from "@/lib/findParam";
import { MANAGE_CONFIGURATION } from "./contract";
import { useDialog } from "./useDialog";

/** Toolbar filters kept in the URL, and the rows they keep. */
export interface DirectoryFilters<Row extends object> {
  /** The compact `Select`s. */
  controls: ReactNode;
  /** Whether any filter is set, which offers "Clear filters". */
  isSet: boolean;
  /** Keep stable between renders while the filters do not change (`useMemo`). */
  keep: (row: Row) => boolean;
  /**
   * The rows the count's total is taken from, when some are out of scope until asked for
   * (retired records). Stable between renders, like `keep`.
   */
  scope?: (row: Row) => boolean;
  clear: () => void;
  /** Scope excluded from the denominator, such as retired destinations. */
  scopeLabel?: string;
  moreFilters?: { content: ReactNode; applied?: string };
}

interface DirectoryProps<Row extends object> {
  /** The page's `h1`, equal to its navigation label, and the grid's name. */
  title: string;
  description?: string;
  /** Fields searched, shown as the Find placeholder. */
  findPlaceholder?: string;
  /**
   * What the directory holds, lower case: plural and singular. `named` is the full plural
   * for loading, error and empty messages when `many` is shortened ("delivery destinations").
   */
  noun: { many: string; one: string; named?: string };
  /** The engine's whole collection; its worst case is within NFR-33, so the UI filters it. */
  query: UseQueryResult<Row[], CommandError>;
  columns: readonly DataGridColumn<Row>[];
  getRowId: (row: Row) => string;
  /** The identifiers and names Find searches; keep stable between renders. */
  findFields: (row: Row) => readonly (string | null)[];
  filters?: DirectoryFilters<Row>;
  /** "Add …" in the header for `manage_configuration`, its dialog, and the empty state's text. */
  add: { label: string; emptyText: string; dialog: (onClose: () => void) => ReactNode };
  onRowAction: (row: Row, order: readonly string[]) => void;
  /** The "⋯" menu of each row, from the record's `allowedActions`. */
  rowActions?: GridRowActions<Row>;
  /** The record open in the side panel, if the directory has one. */
  openRowId?: string | null;
  /** The panel's `followOrder`, so its Previous and Next follow a sort or find. */
  onOrderChange?: (order: readonly string[]) => void;
  panel?: ReactNode;
  /** Shown under the header, such as a failed secondary load. */
  notice?: ReactNode;
}

/**
 * "Clear filters" for several URL filters at once, which one `useSearchParam` setter per
 * filter cannot do (each starts from the address as rendered). Like `useSearchParam`, it
 * replaces the history entry and keeps the other parameters and the navigation state.
 */
export function useClearParams(names: readonly string[]): () => void {
  const [, setParams] = useSearchParams();
  const { state } = useLocation();
  return () =>
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        for (const name of names) next.delete(name);
        return next;
      },
      { replace: true, state },
    );
}

/** For rows whose identifier is `id`. */
export const byId = (row: { id: string }) => row.id;

/**
 * A configuration directory: the header with "Add …", a compact Find/filter/count toolbar,
 * and one grid filling the page whose rows open their record. "No … yet" with the Add action
 * when nothing exists, and "No … match" when Find or a filter excludes everything.
 */
export function Directory<Row extends object>(props: DirectoryProps<Row>) {
  const { title, add } = props;
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const dialog = useDialog();
  const addButton = canManage && (
    <Button variant="primary" onPress={dialog.open}>
      {add.label}
    </Button>
  );
  return (
    <Page layout="collection">
      <PageHeader title={title} description={props.description} actions={addButton} />
      {props.notice}
      <DetailLayout panel={props.panel}>
        <DirectoryList {...props} addButton={addButton} />
      </DetailLayout>
      {dialog.isOpen && add.dialog(dialog.close)}
    </Page>
  );
}

function DirectoryList<Row extends object>({
  title,
  noun,
  query: collection,
  add,
  addButton,
  columns,
  getRowId,
  findFields,
  findPlaceholder,
  filters,
  onRowAction,
  rowActions,
  openRowId,
  onOrderChange,
}: DirectoryProps<Row> & { addButton: ReactNode }) {
  const [query, setQuery] = useFindParam();
  const rows = collection.data;
  const keep = filters?.keep;
  const scope = filters?.scope;
  const inScope = useMemo(() => (scope ? rows?.filter(scope) : rows) ?? [], [rows, scope]);
  const shown = useMemo(
    () => findRows(keep ? inScope.filter(keep) : inScope, query, findFields),
    [inScope, keep, query, findFields],
  );
  const clear = () => (query.trim() === "" ? filters?.clear() : setQuery(""));
  const named = noun.named ?? noun.many;
  return (
    <>
      <Toolbar
        variant="collection"
        moreFilters={filters?.moreFilters}
        actions={
          filters?.isSet && (
            <Button variant="quiet" onPress={filters.clear}>
              Clear filters
            </Button>
          )
        }
        count={
          rows &&
          !collection.isError && (
            <>
              <FindCount
                shown={shown.length}
                total={inScope.length}
                noun={noun.many}
                one={noun.one}
              />
              {filters?.scopeLabel && <> · {filters.scopeLabel}</>}
            </>
          )
        }
      >
        <FindField
          label={`Find ${noun.many}`}
          placeholder={findPlaceholder}
          value={query}
          onChange={setQuery}
        />
        {filters?.controls}
      </Toolbar>
      <QueryContent
        query={collection}
        what={named}
        loadingRows={8}
        empty={{
          when: (answer) =>
            answer.length === 0 || (inScope.length === 0 && !filters?.isSet && query.trim() === ""),
          title: rows?.length ? `No ${noun.many} in this scope` : `No ${named} yet`,
          description: add.emptyText,
          action: addButton,
        }}
      >
        {() =>
          shown.length === 0 ? (
            <NoResults noun={noun.many} query={query} onClear={clear} />
          ) : (
            <DataGrid
              label={title}
              columns={columns}
              rows={shown}
              getRowId={getRowId}
              onRowAction={onRowAction}
              rowActions={rowActions}
              openRowId={openRowId}
              onOrderChange={onOrderChange}
              fill
            />
          )
        }
      </QueryContent>
    </>
  );
}
