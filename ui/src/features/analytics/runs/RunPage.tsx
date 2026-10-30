import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { RefreshControl } from "@/components/RefreshControl";
import { StatusBadge } from "@/components/StatusBadge";
import type { CloseRunInput } from "@/generated/CloseRunInput";
import type { RunDetail } from "@/generated/RunDetail";
import { labelOf } from "@/lib/choices";
import { useCommandMutation, useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import { useRecordSequence } from "@/lib/recordSequence";
import { GET_RUN, MANAGE_QUALITY, RUN_QUERIES } from "../contract";
import { lotStatus, lotText, MATERIAL_KINDS, runStatus, runTitle } from "../labels";
import { RUNS_LIST, runPath, runQcPath } from "../paths";
import { PerformingSite } from "./PerformingSite";
import { Positions } from "./Positions";
import runs from "./runs.module.css";

/**
 * One run: its instrument, method version and lots, and its positions with their attempt
 * kinds (REQ-ANA-001). Analysts add patient samples by scanning container labels and add
 * control, calibrator and blank positions; the engine rechecks every gate on each addition.
 * A run is about many specimens, so it shows no patient. Opened from Runs, it keeps the
 * list in its breadcrumb with Previous and Next in the list's order. The state sits beside
 * the title and the facts stay short, so the positions start within the first screen.
 */
export function RunPage() {
  const { runId = "" } = useParams();
  const formatTime = useFormatTime();
  const person = usePersonText();
  const run = useCommandQuery<RunDetail>(GET_RUN, { input: { runId } });
  const { list, sequence } = useRecordSequence(runId, RUNS_LIST, runPath);
  const detail = run.data;
  const title = runTitle(detail);
  const status = detail && runStatus(detail.state);
  return (
    <Page>
      <PageHeader
        title={title}
        status={status && <StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
        description={
          detail &&
          `Created ${formatTime(detail.createdAt)} by ${person(detail.createdBy, detail.createdByDisplay)}.`
        }
        breadcrumbs={[list, { label: title, href: runPath(runId) }]}
        sequence={sequence}
        actions={
          <>
            <RefreshControl query={run} />
            {detail && <RunActions run={detail} />}
          </>
        }
      />
      <QueryContent query={run} what="run">
        {(loaded) => <RunContent run={loaded} />}
      </QueryContent>
    </Page>
  );
}

function RunActions({ run }: { run: RunDetail }) {
  const navigate = useNavigate();
  const canReviewQc = useHasCapability(MANAGE_QUALITY);
  const [isClosing, setClosing] = useState(false);
  return (
    <>
      {canReviewQc && <Button onPress={() => navigate(runQcPath(run.runId))}>QC review</Button>}
      {run.allowedActions.includes("close_run") && (
        <Button onPress={() => setClosing(true)}>Close run</Button>
      )}
      {isClosing && <CloseRunDialog run={run} onClose={() => setClosing(false)} />}
    </>
  );
}

function RunContent({ run }: { run: RunDetail }) {
  const formatTime = useFormatTime();
  const closed =
    run.closedAt === null
      ? []
      : [
          {
            label: "Closed",
            value: `${formatTime(run.closedAt)}${run.closeReason ? `: ${run.closeReason}` : ""}`,
          },
        ];
  return (
    <>
      <Facts
        layout="compact"
        items={[...closed, { label: "Lots", value: <LotList run={run} /> }]}
      />
      <PerformingSite run={run} />
      <Positions run={run} />
    </>
  );
}

function LotList({ run }: { run: RunDetail }) {
  if (run.lots.length === 0) return "None recorded";
  return (
    <ul className={runs.lots}>
      {run.lots.map((lot) => {
        const status = lotStatus(lot);
        return (
          <li key={lot.lotId}>
            {labelOf(MATERIAL_KINDS, lot.materialKind)}: {lotText(lot)}, expires {lot.expiresOn}{" "}
            <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
          </li>
        );
      })}
    </ul>
  );
}

/** Closing is final, so it asks why; the reason shows beside the closing time. */
function CloseRunDialog({ run, onClose }: { run: RunDetail; onClose: () => void }) {
  const close = useCommandMutation<RunDetail, { input: CloseRunInput }>("close_run", {
    invalidates: RUN_QUERIES,
  });
  return (
    <ReasonConfirmDialog<CloseRunInput>
      title="Close run"
      confirmLabel="Close run"
      variant="danger"
      command={close}
      input={(reason) => ({ runId: run.runId, expectedRevision: run.revision, reason })}
      successTitle="Run closed"
      onClose={onClose}
    >
      <p>
        No more positions can be added to this run. Results that arrive for it after closing are
        quarantined for review instead of being imported.
      </p>
    </ReasonConfirmDialog>
  );
}
