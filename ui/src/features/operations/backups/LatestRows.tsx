import type { UseQueryResult } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Link } from "react-router";
import { Timestamp } from "@/components/Timestamp";
import type { CommandError } from "@/generated/CommandError";
import { backupKind, jobState } from "../labels";
import { TAB_PATH } from "./tabPath";
import { useBackupRuns, useRestoreTests } from "./useBackupLists";

/** The newest row of a list the engine answers newest first, or why there is none. */
function Latest<Row>({
  query,
  rows,
  none,
  children,
}: {
  query: Pick<UseQueryResult<unknown, CommandError>, "isError" | "isPending">;
  rows: readonly Row[];
  none: string;
  children: (row: Row) => ReactNode;
}) {
  const [first] = rows;
  if (query.isError) return "Could not be read";
  if (query.isPending) return "Loading…";
  return first ? children(first) : none;
}

export function LatestRun() {
  const { query, rows } = useBackupRuns();
  return (
    <Latest query={query} rows={rows} none="No backup runs yet">
      {(run) => (
        <Link to={`${TAB_PATH}/runs`}>
          {backupKind(run.kind)} backup, {jobState(run.state).label.toLowerCase()},{" "}
          <Timestamp value={run.requestedAt} />
        </Link>
      )}
    </Latest>
  );
}

export function LatestRestoreTest() {
  const { query, rows } = useRestoreTests();
  return (
    <Latest query={query} rows={rows} none="No restore tests yet">
      {(test) => (
        <Link to={`${TAB_PATH}/restore-tests`}>
          {test.targetLabel}, {jobState(test.state).label.toLowerCase()},{" "}
          <Timestamp value={test.requestedAt} />
        </Link>
      )}
    </Latest>
  );
}
