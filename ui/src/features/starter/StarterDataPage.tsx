import { Link } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import { CATALOG_REVIEW_PATH } from "@/lib/catalogQueries";
import { CatalogSection } from "./CatalogSection";
import { DemoSection } from "./DemoSection";

/**
 * Starter data (REQ-OPS-014, -016): load the Standard Catalog and demo data, then continue
 * to the laboratory's source-independent catalog review (REQ-OPS-015).
 */
export function StarterDataPage() {
  return (
    <Page>
      <PageHeader
        title="Starter data"
        description="Load starter catalog content as drafts, and demo data in the demo laboratory."
      />
      <CatalogSection />
      <DemoSection />
      <p>
        Review and approve catalog drafts from any source in{" "}
        <Link to={CATALOG_REVIEW_PATH}>Catalog review</Link>.
      </p>
    </Page>
  );
}
