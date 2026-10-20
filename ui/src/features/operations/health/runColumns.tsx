import type { DataGridColumn } from "@/components/DataGrid";
import { Timestamp } from "@/components/Timestamp";
import type { AppRunView } from "@/generated/AppRunView";
import { Person } from "../Person";
import { NONE, stopKindText } from "./healthLabels";

function Acknowledged({ run }: { run: AppRunView }) {
  if (run.acknowledged === null) return NONE;
  return (
    <>
      <Person id={run.acknowledged.by} />, <Timestamp value={run.acknowledged.at} />
    </>
  );
}

/** Each start of the app, with how it stopped and any time it was not listening. */
export const RUN_COLUMNS: readonly DataGridColumn<AppRunView>[] = [
  {
    id: "started",
    header: "Started",
    identifier: true,
    cell: (run) => <Timestamp value={run.startedAt} />,
    sortValue: (run) => run.startedAt,
    width: 190,
  },
  {
    id: "stopped",
    header: "Stopped",
    cell: (run) => (run.stoppedAt ? <Timestamp value={run.stoppedAt} /> : NONE),
    sortValue: (run) => run.stoppedAt ?? "",
    width: 190,
  },
  { id: "stopKind", header: "How it stopped", cell: (run) => stopKindText(run.stopKind) },
  { id: "version", header: "App version", cell: (run) => run.appVersion, width: 110 },
  { id: "webview", header: "Web engine", cell: (run) => run.webview ?? NONE },
  {
    id: "gap",
    header: "Gap before start",
    cell: (run) => (run.gapSeconds === null ? NONE : `${run.gapSeconds} s`),
    sortValue: (run) => run.gapSeconds ?? -1,
    align: "end",
    width: 140,
  },
  {
    id: "clockGap",
    header: "Clock gaps",
    cell: (run) => (run.clockGapSeconds === null ? NONE : `${run.clockGapSeconds} s`),
    sortValue: (run) => run.clockGapSeconds ?? -1,
    align: "end",
    width: 120,
  },
  {
    id: "unclean",
    header: "Previous stop unclean",
    cell: (run) => (run.previousUnclean ? "Yes" : "No"),
    width: 170,
  },
  { id: "acknowledged", header: "Gaps acknowledged", cell: (run) => <Acknowledged run={run} /> },
];
