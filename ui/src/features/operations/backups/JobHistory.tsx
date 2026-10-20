import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { DetailLayout, DetailPanel } from "@/components/DetailPanel";
import { ErrorState } from "@/components/ErrorState";
import { ListCount } from "@/components/ListLimit";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Toolbar } from "@/components/Toolbar";
import type { CommandError } from "@/generated/CommandError";
import type { CommandName } from "@/generated/CommandName";
import { type OpenRecord, useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { useExactJob } from "./useExactJob";

type Navigation = Pick<OpenRecord, "close" | "previous" | "next">;

/** Shared backup/restore history: loaded pages, exact links, job progress and recovery. */
export function JobHistory<Row extends { state: string }>({
  query,
  rows,
  hasMore,
  command,
  idField,
  getRowId,
  columns,
  noun,
  singular,
  label,
  emptyDescription,
  renderPanel,
}: {
  query: UseInfiniteQueryResult<InfiniteData<Row[]>, CommandError>;
  rows: Row[];
  hasMore: boolean;
  command: CommandName;
  idField: string;
  getRowId: (row: Row) => string;
  columns: readonly DataGridColumn<Row>[];
  noun: string;
  singular: string;
  label: string;
  emptyDescription: string;
  renderPanel: (row: Row, navigation: Navigation) => ReactNode;
}) {
  const record = useOpenRecord();
  const outside = Boolean(record.openId) && !rows.some((row) => getRowId(row) === record.openId);
  const exact = useExactJob<Row>(
    command,
    { input: { limit: 1, [idField]: record.openId } },
    outside,
  );
  const selected = exact.data?.find((row) => getRowId(row) === record.openId);
  const panelRows = [...rows, ...(outside && selected ? [selected] : [])];
  const refresh = () => {
    void query.refetch();
    if (outside) void exact.refetch();
  };
  const panel =
    outside && !selected ? (
      <DetailPanel identifier={`Selected ${singular}`} onClose={record.close}>
        <QueryContent
          query={exact}
          what={`selected ${singular}`}
          empty={{
            when: (data) => data.length === 0,
            title: "Record unavailable",
            description: "The selected record was not found. Refresh or close this panel.",
          }}
        >
          {() => <p>The selected record is unavailable. Refresh the history.</p>}
        </QueryContent>
      </DetailPanel>
    ) : (
      openRecordPanel(record, panelRows, getRowId, (row) => renderPanel(row, record))
    );
  return (
    <DetailLayout
      panel={
        panel ? (
          <>
            {outside && selected && exact.isRefetchError && (
              <ErrorState
                title={`Could not refresh the selected ${singular}. The retained record may be out of date.`}
                error={exact.error}
                onRetry={() => void exact.refetch()}
              />
            )}
            {panel}
          </>
        ) : null
      }
    >
      <Toolbar
        variant="collection"
        actions={
          <RefreshControl
            query={{
              ...query,
              isFetching: query.isFetching || (outside && exact.isFetching),
              refetch: refresh,
            }}
          />
        }
        count={
          query.isSuccess && (
            <ListCount shown={rows.length} noun={noun} one={singular} hasMore={hasMore} />
          )
        }
      />
      <QueryContent
        query={query}
        what={noun}
        loadingRows={4}
        empty={{
          when: (data) => data.pages.every((page) => page.length === 0),
          title: `No ${noun}`,
          description: emptyDescription,
        }}
      >
        {() => (
          <>
            {hasMore && (
              <Button
                variant="secondary"
                isPending={query.isFetchingNextPage}
                isDisabled={query.isFetching}
                onPress={() => void query.fetchNextPage()}
              >
                Load older {noun}
              </Button>
            )}
            <DataGrid
              label={label}
              columns={columns}
              rows={rows}
              getRowId={getRowId}
              openRowId={record.openId}
              onOrderChange={record.followOrder}
              onRowAction={(row, order) => record.open(getRowId(row), order)}
              fill
            />
          </>
        )}
      </QueryContent>
    </DetailLayout>
  );
}
