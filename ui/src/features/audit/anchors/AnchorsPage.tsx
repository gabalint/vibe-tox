import { Link, useSearchParams } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import { Tabs } from "@/components/Tabs";
import { AnchorExportsSection } from "./AnchorExportsSection";
import { AnchorStatusSection } from "./AnchorStatusSection";
import { VerifyAnchorsSection } from "./VerifyAnchorsSection";

const TABS = [
  { id: "verify", label: "Verify anchors", href: "/audit/anchors" },
  { id: "files", label: "Anchor files", href: "/audit/anchors?tab=files" },
];

/**
 * Audit anchors (NFR-18, ADR 0008), for people with `view_audit`: the audit trail is sealed at
 * intervals and each seal is copied as an anchor to the backup targets, so a later change on
 * this computer shows when the trail is checked against them. Exporting now needs
 * `backup_restore`.
 */
export function AnchorsPage() {
  const [params] = useSearchParams();
  const tab = params.get("tab") === "files" ? "files" : "verify";
  return (
    <Page>
      <PageHeader
        title="Audit anchors"
        description="Fingerprints of the audit trail kept on the backup targets, so later changes on this computer can be detected."
        actions={<Link to="/audit">Audit trail</Link>}
      />
      <AnchorStatusSection />
      <Tabs
        label="Audit anchors"
        tabs={TABS}
        selectedId={tab}
        contentLayout={tab === "files" ? "collection" : undefined}
      >
        {tab === "files" ? <AnchorExportsSection /> : <VerifyAnchorsSection />}
      </Tabs>
    </Page>
  );
}
