import { Navigate, useParams } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import { useCommandQuery } from "@/lib/commands";
import { LIST_CLASS_MAPPING_VERSIONS } from "../contract";
import { classMappingPath } from "../paths";

/**
 * The address of one version by its ID (`/billing/class-mappings/<versionId>`), kept so
 * earlier links still work: it shows that version on the class mapping's page
 * (`?version=<number>`), replacing this address.
 */
export function ClassMappingPage() {
  const { versionId = "" } = useParams();
  const versions = useCommandQuery<BillingClassMappingVersionSummary[]>(
    LIST_CLASS_MAPPING_VERSIONS,
  );
  return (
    <Page>
      <PageHeader title="Billing class mappings" />
      <QueryContent
        query={versions}
        what="class mappings"
        empty={{
          when: (rows) => !rows.some((version) => version.versionId === versionId),
          title: "Class mapping version not found",
          description: "No version of the class mapping has this ID.",
        }}
      >
        {(rows) => {
          const version = rows.find((each) => each.versionId === versionId);
          return version && <Navigate replace to={classMappingPath(version.version)} />;
        }}
      </QueryContent>
    </Page>
  );
}
