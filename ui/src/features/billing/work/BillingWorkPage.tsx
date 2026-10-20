import { useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { ListCount, ListLimit } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import type { BillingWorkCursor } from "@/generated/BillingWorkCursor";
import type { BillingWorkItem } from "@/generated/BillingWorkItem";
import { choiceOf, labelOf } from "@/lib/choices";
import { useCommandPages } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import { LIST_BILLING_WORK, LIST_LIMIT } from "../contract";
import { WORK_FILTERS, type WorkFilter } from "../labels";
import { derivationPath, handoffPath } from "../paths";
import { CreateHandoffAction } from "./CreateHandoffAction";
import { DERIVE_SERVICES, useWorkRowActions } from "./useWorkRowActions";
import { workColumns } from "./workColumns";

const DEFAULT_FILTER: WorkFilter = "needs_derivation";
const LIST_LABEL = "Billing work";
const NO_ROWS: readonly BillingWorkItem[] = [];

function workEntryId(row: BillingWorkItem): string {
  return `${row.subjectId}|${row.dateOfService ?? ""}|${row.derivationId ?? ""}`;
}

/**
 * The entry a work-queue item names (`?open=`): a handed-off item by its ID, or work not yet
 * derived by its patient and exact collection date; an unknown date is explicit.
 */
function namedEntry(rows: readonly BillingWorkItem[], named: string | null) {
  if (named === null) return null;
  const row = rows.find(
    (each) =>
      each.itemId === named ||
      (each.derivationId === null && `${each.subjectId}|${each.dateOfService ?? ""}` === named),
  );
  return row ? workEntryId(row) : null;
}

/**
 * Billing work by what it needs (REQ-BIL-002, REQ-BIL-005), a queue: patients and dates of
 * service to derive, derivations held or ready to hand off, and handed-off items awaiting or
 * rejected by the billing partner, oldest first. The filter stays in the address. A row opens
 * its handoff once handed off, otherwise its derivation, and work not yet derived opens
 * Derive services; its menu holds the actions the engine allows on it. Handing off several derivations at once is the one bulk action.
 */
export function BillingWorkPage() {
  const [, setParams] = useSearchParams();
  const [filterParam, setFilter] = useSearchParam("filter");
  const [named] = useSearchParam("open");
  const filter = choiceOf(WORK_FILTERS, filterParam) ?? DEFAULT_FILTER;
  const [subjectId] = useSearchParam("subjectId");
  const [dateOfService] = useSearchParam("dateOfService");
  const target = subjectId ? { subjectId, dateOfService: dateOfService || null } : null;
  const work = useCommandPages<BillingWorkItem, BillingWorkCursor>(LIST_BILLING_WORK, {
    key: { filter, target },
    pageArgs: (after) => ({ input: { filter, limit: LIST_LIMIT, target, after: after ?? null } }),
    nextCursor: (rows) => {
      const last = rows.at(-1);
      return rows[0]?.hasMore && last
        ? { since: last.since, subjectId: last.subjectId, dateOfService: last.dateOfService }
        : undefined;
    },
  });
  const rows = work.data
    ? [
        ...new Map(
          work.data.pages.flat().map((row) => [`${row.subjectId}|${row.dateOfService ?? ""}`, row]),
        ).values(),
      ]
    : NO_ROWS;
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const selected = rows.filter((row) => selectedIds.has(workEntryId(row)));
  const actions = useWorkRowActions();
  const open = useOpenRow(rows, (row) => actions.start(DERIVE_SERVICES, row));
  // A state every row shares is the filter's, so it shows as plain text.
  const columns = useMemo(
    () => workColumns(rows.every((row) => row.state === rows[0]?.state)),
    [rows],
  );
  return (
    <Page layout="collection">
      <PageHeader
        title={LIST_LABEL}
        description="Derive billable services from final reports and hand them to the billing partner."
      />
      <Toolbar
        variant="collection"
        count={
          work.isSuccess && (
            <ListCount
              shown={rows.length}
              noun="billing work items"
              one="billing work item"
              hasMore={work.hasNextPage}
            />
          )
        }
      >
        <Select
          label="Show"
          layout="inline"
          name="filter"
          options={WORK_FILTERS}
          value={filter}
          onChange={(next) => {
            setSelectedIds(new Set());
            setFilter(choiceOf(WORK_FILTERS, next) ?? DEFAULT_FILTER);
          }}
        />
        <RefreshControl query={work} />
      </Toolbar>
      {target && (
        <p>
          Showing the selected patient’s work for{" "}
          {target.dateOfService ?? "an unknown collection date"}.{" "}
          <Button
            onPress={() => {
              setSelectedIds(new Set());
              setParams(
                (current) => {
                  const next = new URLSearchParams(current);
                  next.delete("subjectId");
                  next.delete("dateOfService");
                  next.delete("open");
                  return next;
                },
                { replace: true },
              );
            }}
          >
            Show all patients and dates
          </Button>
        </p>
      )}
      {work.isSuccess && (
        <ListLimit variant="summary" shown={rows.length} end="oldest" hasMore={work.hasNextPage} />
      )}
      <QueryContent
        query={work}
        what="billing work"
        loadingRows={8}
        empty={{
          when: () => rows.length === 0,
          // Every filter is one kind of work; there is no "all" to fall back to.
          title: `Nothing in “${labelOf(WORK_FILTERS, filter)}”`,
          description: "Choose another kind of work under Show.",
        }}
      >
        {() => (
          <>
            <DataGrid
              label={LIST_LABEL}
              columns={columns}
              rows={rows}
              getRowId={workEntryId}
              onRowAction={open}
              openRowId={namedEntry(rows, named)}
              rowActions={actions.rowActions}
              bulk={
                // Only approved derivations, listed under "ready", are handed off together.
                filter === "ready"
                  ? {
                      selectedIds,
                      onChange: setSelectedIds,
                      actions: <CreateHandoffAction selected={selected} />,
                    }
                  : undefined
              }
              fill
            />
          </>
        )}
      </QueryContent>
      {work.hasNextPage && (
        <Button isPending={work.isFetchingNextPage} onPress={() => void work.fetchNextPage()}>
          Load more billing work
        </Button>
      )}
      {actions.dialog}
    </Page>
  );
}

/**
 * Opens a row's record page: its handoff once handed off, otherwise its derivation. Work
 * still to derive has neither, so its usual next step, deriving, is the row's action when
 * the engine allows it. Previous and Next walk the listed records of the same kind.
 */
function useOpenRow(rows: readonly BillingWorkItem[], derive: (row: BillingWorkItem) => void) {
  const openHandoff = useOpenRecordPage(handoffPath, LIST_LABEL);
  const openDerivation = useOpenRecordPage(derivationPath, LIST_LABEL);
  return (row: BillingWorkItem, order: readonly string[]) => {
    const byId = new Map(rows.map((each) => [workEntryId(each), each]));
    const listed = order.flatMap((id) => byId.get(id) ?? []);
    const unique = (ids: readonly (string | null)[]) =>
      [...new Set(ids)].filter((id): id is string => id !== null);
    if (row.handoffId) {
      openHandoff(row.handoffId, unique(listed.map((each) => each.handoffId)));
    } else if (row.derivationId) {
      const derivations = listed.filter((each) => each.handoffId === null);
      openDerivation(row.derivationId, unique(derivations.map((each) => each.derivationId)));
    } else if (row.allowedActions.includes(DERIVE_SERVICES)) {
      derive(row);
    }
  };
}
