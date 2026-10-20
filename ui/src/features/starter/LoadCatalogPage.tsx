import { useState } from "react";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import type { StandardCatalogLoaded } from "@/generated/StandardCatalogLoaded";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import { useCommandQuery } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { CatalogChooser } from "./CatalogChooser";
import { LOAD_CATALOG_PATH, STANDARD_CATALOG_TREE, STARTER_DATA_PATH } from "./contract";
import { LoadResult } from "./LoadResult";

const BREADCRUMBS = [
  { label: "Starter data", href: STARTER_DATA_PATH },
  { label: "Load Standard Catalog", href: LOAD_CATALOG_PATH },
];

/**
 * Load Standard Catalog (proposed REQ-OPS-014), a task flow for `manage_configuration`: choose
 * from the shipped file, read the summary and the disclaimer, then load the choices as drafts.
 * The engine computes what the choices need and refuses anything unavailable.
 */
export function LoadCatalogPage() {
  const tree = useCommandQuery<StandardCatalogTree>(STANDARD_CATALOG_TREE);
  const [loaded, setLoaded] = useState<StandardCatalogLoaded | null>(null);
  return (
    <Page>
      <PageHeader
        title="Load Standard Catalog"
        breadcrumbs={BREADCRUMBS}
        description="Choose tests, panels and methods. What they need is selected with them. Everything loads as drafts for review."
      />
      <QueryContent query={tree} what="Standard Catalog" loadingRows={8}>
        {(data) => (
          <div className={layout.stack}>
            <p className={layout.hint}>
              Standard Catalog version {data.info.catalogVersion}
              {data.info.reviewStatus === "unreviewed" && ": unreviewed starter content"}. LOINC{" "}
              {data.info.loincVersion}.
            </p>
            {loaded ? (
              <LoadResult result={loaded} />
            ) : (
              <CatalogChooser
                tree={data}
                onLoaded={setLoaded}
                onReload={() => void tree.refetch()}
              />
            )}
            <p className={layout.hint}>{data.info.loincNotice}</p>
          </div>
        )}
      </QueryContent>
    </Page>
  );
}
