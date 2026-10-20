import { useLocation, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { PageHeader } from "@/components/Page";
import { StatusBadge } from "@/components/StatusBadge";
import { Tabs } from "@/components/Tabs";
import { useRecordSequence } from "@/lib/recordSequence";
import { GallerySection } from "./GallerySection";

const TABS = [
  { id: "content", label: "Content" },
  { id: "versions", label: "Versions" },
  { id: "used-by", label: "Used by" },
];

/**
 * A record page's header and tabs. Open a row in "Find in a directory" to get the breadcrumb
 * back to the list as it was, and Previous and Next in its order. In the gallery the record
 * and tab live in the search (`?record=`, `?tab=`); on a screen each tab is a path segment.
 * This demo's header adds a second `h1` to the gallery page.
 */
export function NavigationSection() {
  const { pathname } = useLocation();
  const [params] = useSearchParams();
  const record = params.get("record") ?? "SYN-AN-01";
  const tab = params.get("tab") ?? "content";
  const recordPath = (id: string) => `${pathname}?record=${id}`;
  const home = { label: "Synthetic analytes", href: pathname };
  const { list, sequence, keep } = useRecordSequence(record, home, recordPath);
  return (
    <GallerySection title="Record page header and tabs">
      <PageHeader
        title={`${record} Synthetic analyte`}
        breadcrumbs={[list, { label: record, href: recordPath(record) }]}
        status={<StatusBadge tone="warning">Draft awaiting approval</StatusBadge>}
        sequence={sequence}
        actions={<Button variant="primary">Approve draft</Button>}
      />
      <Tabs
        label={`Synthetic analyte ${record}`}
        tabs={TABS.map((each) => ({ ...each, href: `${recordPath(record)}&tab=${each.id}` }))}
        selectedId={tab}
        routerOptions={keep}
      >
        <p>
          Synthetic {tab} of {record}.
        </p>
      </Tabs>
    </GallerySection>
  );
}
