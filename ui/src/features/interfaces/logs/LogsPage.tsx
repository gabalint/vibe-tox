import { useContext, useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { Checkbox } from "@/components/Checkbox";
import { ErrorState } from "@/components/ErrorState";
import { FindField } from "@/components/FindField";
import { Page, PageHeader } from "@/components/Page";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Tabs } from "@/components/Tabs";
import { useToast } from "@/components/Toast";
import type { ExportedMessageLog } from "@/generated/ExportedMessageLog";
import type { ListPartnerMessages } from "@/generated/ListPartnerMessages";
import type { MessageView } from "@/generated/MessageView";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerMessagePage } from "@/generated/PartnerMessagePage";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { optionalText } from "@/lib/choices";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { useFindParam } from "@/lib/findParam";
import { calendarDayToday, LabTimeZoneContext } from "@/lib/format";
import { useRouteTabs } from "@/lib/routeTabs";
import { EXPORT_MESSAGES, LIST_CONNECTIONS, LIST_MESSAGES } from "../contract";
import { LOGS_PATH } from "../paths";
import styles from "./LogsPage.module.css";
import { MessagesGrid } from "./MessagesSection";

type Row = PartnerMessageRow;

/**
 * The tabs of the mockup. Only an MLLP partner answers: what it rejected (AE, AR) shows under
 * Rejected, with received orders the LIS refused, until a person resolves it; what it has not
 * acknowledged yet shows under Awaiting ACK; a folder or SFTP server never answers. Duplicates
 * hold orders read before.
 */
const TABS = [
  { id: "all", label: "All" },
  { id: "failed", label: "Failed" },
  { id: "rejected", label: "Rejected" },
  { id: "awaiting_ack", label: "Awaiting ACK" },
  { id: "outcome_unknown", label: "Outcome unknown" },
  { id: "duplicates", label: "Duplicates" },
] as const;
type TabId = (typeof TABS)[number]["id"];

const IN_TAB: Record<TabId, (row: Row) => boolean> = {
  all: () => true,
  failed: (row) => row.transport === "failed",
  rejected: (row) =>
    row.rejectionResolved === null &&
    (row.received === "rejected" || row.ackCode === "AE" || row.ackCode === "AR"),
  awaiting_ack: (row) => row.awaitingAck,
  outcome_unknown: (row) => row.outcomeUnknown,
  duplicates: (row) => row.received === "duplicate",
};

const PERIODS = [
  { id: "today", label: "Today" },
  { id: "7", label: "Last 7 days" },
  { id: "30", label: "Last 30 days" },
  { id: "all", label: "All time" },
];
const DIRECTIONS = [
  { id: "both", label: "In and out" },
  { id: "out", label: "Out" },
  { id: "in", label: "In" },
];
const OUTCOMES = [
  { id: "all", label: "All outcomes" },
  { id: "written", label: "Written" },
  { id: "failed", label: "Failed" },
  { id: "not_sent", label: "Not sent" },
  { id: "received", label: "Received" },
];

/** Whether the row goes the chosen way: received orders come in, everything else goes out. */
const inDirection = (row: Row, direction: string) =>
  direction === "both" || (direction === "in") === (row.received !== null);
const DAY_MS = 24 * 60 * 60 * 1000;

/** The engine's timestamp form: nine fractional digits. */
const utc = (date: Date) => date.toISOString().replace(/\.(\d{3})Z$/, ".$1000000Z");

/** Midnight today in the laboratory's time zone, or the computer's without one. */
function startOfLabDay(now: Date, timeZone: string | null): Date {
  const offset =
    new Intl.DateTimeFormat("en-US", {
      timeZone: timeZone ?? undefined,
      timeZoneName: "longOffset",
    })
      .formatToParts(now)
      .find((part) => part.type === "timeZoneName")?.value ?? "GMT";
  return new Date(`${calendarDayToday(now, timeZone)}T00:00:00${offset.slice(3) || "Z"}`);
}

/** The engine's view for the rows shown, so Export CSV saves exactly them. */
function exportView(tab: TabId, outcome: string): MessageView {
  if (outcome === "written") return "outcome_unknown";
  if (outcome === "failed" || outcome === "not_sent") return outcome;
  return tab === "failed" || tab === "outcome_unknown" ? tab : "all";
}

/**
 * Logs (ticket #4555; REQ-INT-002, REQ-RPT-003, REQ-RPT-011), as the Interfaces mockup: every
 * message in and out, with two separate outcomes, transport and application, and never a bare
 * delivery claim. Period, connection, direction, outcome, Show test messages (off: the engine
 * leaves test messages out, tab counts included, #4679) and search sit in one row; the tabs
 * below count what they hold. Filters stay in the URL, the tab in the path. Refresh reads the
 * list again with the same filters, tab and search, and says when it was read (#4728). Export
 * CSV saves the rows shown to the Downloads folder, or Export N selected only the rows checked
 * (#4682); it is audited. The search takes several values separated by commas.
 */
export function LogsPage() {
  const toast = useToast();
  const labTimeZone = useContext(LabTimeZoneContext);
  const [params, setParams] = useSearchParams();
  const [search, setSearch] = useFindParam();
  const { selectedId: tab, tabs } = useRouteTabs(LOGS_PATH, TABS);
  const period = params.get("period") ?? "today";
  const connectionId = params.get("connection") ?? undefined;
  const direction = params.get("direction") ?? "both";
  const outcome = params.get("outcome") ?? "all";
  const since = useMemo(() => {
    if (period === "all") return undefined;
    const now = new Date();
    return utc(
      period === "today"
        ? startOfLabDay(now, labTimeZone)
        : new Date(now.getTime() - Number(period) * DAY_MS),
    );
  }, [period, labTimeZone]);
  const showTests = params.get("tests") === "shown";
  const searched = optionalText(search) ?? undefined;
  // Hidden in the engine, so the tab counts leave them out too.
  const input: ListPartnerMessages = {
    view: "all",
    since,
    connectionId,
    search: searched,
    hideTests: !showTests,
  };
  const messages = useCommandQuery<PartnerMessagePage>(LIST_MESSAGES, { input });
  const connections = useCommandQuery<PartnerConnection[]>(LIST_CONNECTIONS);
  const exportLog = useCommandMutation<ExportedMessageLog, { input: ListPartnerMessages }>(
    EXPORT_MESSAGES,
    { invalidates: [] },
  );
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  const visible = (messages.data?.rows ?? []).filter(
    (row) => inDirection(row, direction) && (outcome === "all" || row.transport === outcome),
  );
  const shown = visible.filter(IN_TAB[tab]);
  const setParam = (key: string, value: string | null, all: string) =>
    setParams((current) => {
      const next = new URLSearchParams(current);
      if (value === null || value === all) next.delete(key);
      else next.set(key, value);
      return next;
    });
  // The rows checked, or with none the rows shown.
  const exported: ListPartnerMessages =
    selectedIds.size > 0
      ? { ...input, rowIds: [...selectedIds] }
      : { ...input, view: exportView(tab, outcome) };
  const runExport = () =>
    exportLog.mutate(
      { input: exported },
      {
        onSuccess: (file) =>
          toast.show({
            tone: "success",
            title: `Saved ${file.fileName} to your Downloads folder.`,
          }),
      },
    );
  return (
    <Page layout="collection">
      <PageHeader
        title="Logs"
        description="Every message in and out, with two separate outcomes: did it leave or arrive (transport), and did the other system accept it (application)."
      />
      <div className={styles.filters}>
        <Select
          label="Period"
          isLabelHidden
          name="period"
          options={PERIODS}
          value={period}
          onChange={(value) => setParam("period", value, "today")}
        />
        <Select
          label="Connection"
          isLabelHidden
          name="connection"
          options={[
            { id: "", label: "All connections" },
            ...(connections.data ?? []).map((row) => ({ id: row.id, label: row.settings.name })),
          ]}
          value={connectionId ?? ""}
          onChange={(value) => setParam("connection", value, "")}
        />
        <Select
          label="Direction"
          isLabelHidden
          name="direction"
          options={DIRECTIONS}
          value={direction}
          onChange={(value) => setParam("direction", value, "both")}
        />
        <Select
          label="Outcome"
          isLabelHidden
          name="outcome"
          options={OUTCOMES}
          value={outcome}
          onChange={(value) => setParam("outcome", value, "all")}
        />
        <Checkbox
          name="tests"
          isSelected={showTests}
          onChange={(shown) => setParam("tests", shown ? "shown" : null, "")}
        >
          Show test messages
        </Checkbox>
        <div className={styles.search}>
          <FindField
            label="Search by report number, handoff number or file name; separate several with commas"
            placeholder="Search report, handoff, file…"
            value={search}
            onChange={setSearch}
          />
        </div>
        <div className={styles.actions}>
          <RefreshControl query={messages} />
          <Button
            isDisabled={shown.length === 0}
            isPending={exportLog.isPending}
            onPress={runExport}
          >
            {selectedIds.size > 0 ? `Export ${selectedIds.size} selected` : "Export CSV"}
          </Button>
        </div>
      </div>
      {exportLog.error && <ErrorState title="Not exported" error={exportLog.error} />}
      <Tabs
        contentLayout="collection"
        label="Outcomes"
        tabs={tabs.map((item) => ({
          ...item,
          label: messages.data
            ? `${item.label} (${visible.filter(IN_TAB[item.id]).length})`
            : item.label,
        }))}
        selectedId={tab}
      >
        <MessagesGrid
          query={messages}
          rows={shown}
          selection={{ selectedIds, onChange: setSelectedIds }}
        />
      </Tabs>
    </Page>
  );
}
