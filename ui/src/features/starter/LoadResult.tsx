import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { StandardCatalogLoaded } from "@/generated/StandardCatalogLoaded";
import { CATALOG_REVIEW_PATH } from "@/lib/catalogQueries";

/** What the load created, with a link to the laboratory's catalog review workflow. */
export function LoadResult({ result }: { result: StandardCatalogLoaded }) {
  const created = result.loadId !== null;
  return (
    <Section title="Loaded">
      <Alert
        tone={created ? "success" : "info"}
        title={created ? "Standard Catalog loaded as drafts" : "Nothing new to load"}
      >
        <Facts
          layout="compact"
          items={[
            { label: "Specimen types", value: String(result.specimenTypes) },
            { label: "Analytes", value: String(result.analytes) },
            { label: "Methods", value: String(result.methods) },
            { label: "Tests and panels", value: String(result.orderables) },
            { label: "Already in your catalog, skipped", value: String(result.skipped) },
          ]}
        />
      </Alert>
      <p>
        Next, generate a Catalog Preview and review its values. Approve the reviewed drafts in
        Catalog review, subject to the laboratory's approval policy.
      </p>
      <p>
        <Link to={CATALOG_REVIEW_PATH}>Go to Catalog review</Link>
      </p>
    </Section>
  );
}
