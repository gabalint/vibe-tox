import { Link } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import { Tabs } from "@/components/Tabs";
import { useRouteTabs } from "@/lib/routeTabs";
import { AppRuns } from "./AppRuns";
import { CheckHistory } from "./CheckHistory";

const PATH = "/operations/health/history";

const TABS = [
  { id: "runs", label: "Application runs" },
  { id: "changes", label: "State changes" },
] as const;

/**
 * Health history (REQ-OPS-008, REQ-OPS-013): each run of the app with its stop and any
 * listening gap before it, and every change of a check's state. The runs come first: a gap
 * waits for someone to acknowledge it. Each tab is a path segment; the base path shows the runs.
 */
export function HealthHistoryPage() {
  const { selectedId: tab, tabs } = useRouteTabs(PATH, TABS);
  return (
    <Page>
      <PageHeader
        title="Health history"
        description="Each start and stop of the app, and changes of each health check."
        actions={<Link to="/operations/health">Host health</Link>}
      />
      <Tabs contentLayout="collection" label="Health history" tabs={tabs} selectedId={tab}>
        {tab === "runs" ? <AppRuns /> : <CheckHistory />}
      </Tabs>
    </Page>
  );
}
