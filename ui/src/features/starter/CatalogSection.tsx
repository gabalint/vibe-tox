import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { LoadingState } from "@/components/LoadingState";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { StandardCatalogStatus } from "@/generated/StandardCatalogStatus";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import {
  LOAD_CATALOG_PATH,
  MANAGE_CONFIGURATION,
  STANDARD_CATALOG_STATUS,
  STANDARD_CATALOG_TREE,
} from "./contract";

/**
 * The Standard Catalog: the shipped version and its review status (read with the tree, which
 * needs `manage_configuration`), what was loaded, and the drafts still awaiting review.
 */
export function CatalogSection() {
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const navigate = useNavigate();
  const status = useCommandQuery<StandardCatalogStatus>(STANDARD_CATALOG_STATUS);
  return (
    <Section
      title="Standard Catalog"
      actions={
        canManage && (
          <Button variant="primary" onPress={() => navigate(LOAD_CATALOG_PATH)}>
            Load Standard Catalog
          </Button>
        )
      }
    >
      <p>
        Specimen types, analytes, methods, tests and panels shipped with the app, loaded as drafts
        that the laboratory checks and approves before patient testing.
      </p>
      {canManage && <ShippedVersion />}
      {status.isPending && <LoadingState label="Loading what was loaded…" />}
      {status.isError && (
        <ErrorState
          title="What was loaded could not be read"
          error={status.error}
          onRetry={() => void status.refetch()}
        />
      )}
      {status.data && <Loads status={status.data} />}
    </Section>
  );
}

/** The version this build ships and whether a clinical reviewer has reviewed it. */
function ShippedVersion() {
  const tree = useCommandQuery<StandardCatalogTree>(STANDARD_CATALOG_TREE);
  if (tree.isError) {
    return <ErrorState title="The Standard Catalog could not be read" error={tree.error} />;
  }
  if (!tree.data) return null;
  const { catalogVersion, reviewStatus } = tree.data.info;
  return (
    <Facts
      layout="compact"
      items={[
        { label: "Version shipped", value: catalogVersion },
        {
          label: "Content review",
          value:
            reviewStatus === "reviewed"
              ? "Reviewed by a clinical reviewer"
              : "Unreviewed starter content",
        },
      ]}
    />
  );
}

function Loads({ status }: { status: StandardCatalogStatus }) {
  if (status.loads.length === 0) {
    return <p className={layout.hint}>Nothing has been loaded from the Standard Catalog.</p>;
  }
  return (
    <>
      <Facts
        layout="compact"
        items={[{ label: "Drafts awaiting review", value: String(status.draftsRemaining) }]}
      />
      <h3 className={layout.subheading}>Loads</h3>
      <ul className={layout.list}>
        {status.loads.map((load) => (
          <li key={load.id}>
            Version {load.catalogVersion}, loaded <Timestamp value={load.loadedAt} />:{" "}
            {load.records} records
          </li>
        ))}
      </ul>
    </>
  );
}
