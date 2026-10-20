import { useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Tabs } from "@/components/Tabs";
import type { RunQcView } from "@/generated/RunQcView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { GET_RUN_QC, PERFORM_ANALYSIS } from "../contract";
import { runPath, runQcPath } from "../paths";
import { AssessmentsTab } from "./AssessmentsTab";
import { ConflictsTab } from "./ConflictsTab";
import { DeviationsTab } from "./DeviationsTab";
import { ObservationGrid } from "./ObservationGrid";
import { useLotName } from "./useLotName";

/** One route per tab (`/bench/runs/:runId/qc/:tab`); the run's QC page shows the first. */
const TABS = [
  { id: "assessments", label: "Assessments" },
  { id: "observations", label: "Observations" },
  { id: "changed-rows", label: "Changed QC rows" },
  { id: "deviations", label: "Deviations" },
] as const;

type TabId = (typeof TABS)[number]["id"];

/** How many of the tab's items are open: changed rows to decide, deviations to close. */
function openCount(id: TabId, view: RunQcView): number {
  if (id === "changed-rows") return view.conflicts.filter((row) => row.state === "open").length;
  if (id === "deviations") return view.deviations.filter((row) => row.state === "open").length;
  return 0;
}

function tabLabel(id: TabId, label: string, view: RunQcView): string {
  const open = openCount(id, view);
  return open === 0 ? label : `${label} (${open} open)`;
}

/**
 * QC of one run (REQ-ANA-004, REQ-ANA-005), one list per tab: the assessment of each
 * analyte, the run and each preparation batch; every control and calibrator observation;
 * changed QC rows awaiting a decision; and deviations. Accepted QC is not result approval.
 */
export function RunQcPage() {
  const { runId = "" } = useParams();
  const { selectedId: selected, tabs } = useRouteTabs(runQcPath(runId), TABS);
  const navigate = useNavigate();
  const canOpenRun = useHasCapability(PERFORM_ANALYSIS);
  const qc = useCommandQuery<RunQcView>(GET_RUN_QC, { input: { runId } });
  return (
    <Page>
      <PageHeader
        title="Run QC review"
        description="Assess each analyte's QC from the observations received for this run."
        actions={canOpenRun && <Button onPress={() => navigate(runPath(runId))}>Open run</Button>}
      />
      <QueryContent query={qc} what="run QC" loadingRows={8}>
        {(view) => (
          <Tabs
            label="Run QC"
            contentLayout="collection"
            tabs={tabs.map((each) => ({ ...each, label: tabLabel(each.id, each.label, view) }))}
            selectedId={selected}
          >
            <QcTab tab={selected} view={view} />
          </Tabs>
        )}
      </QueryContent>
    </Page>
  );
}

function QcTab({ tab, view }: { tab: TabId; view: RunQcView }) {
  const lotName = useLotName();
  if (tab === "changed-rows") return <ConflictsTab view={view} />;
  if (tab === "deviations") return <DeviationsTab view={view} lotName={lotName} />;
  if (tab === "assessments") return <AssessmentsTab view={view} />;
  if (view.analytes.length === 0 && view.unmappedObservations.length === 0) {
    return (
      <EmptyState
        title="No QC observations yet"
        description="No QC observations have been received for this run."
      />
    );
  }
  return <ObservationGrid view={view} lotName={lotName} />;
}
