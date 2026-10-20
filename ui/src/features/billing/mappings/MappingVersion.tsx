import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { BillingClassMappingEntry } from "@/generated/BillingClassMappingEntry";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { GET_CLASS_MAPPING_VERSION, LIST_ANALYTES } from "../contract";
import { MappingActions } from "./MappingActions";
import { MappingFacts } from "./MappingFacts";
import mappings from "./mappings.module.css";

interface EntryRow extends BillingClassMappingEntry {
  analyteName: string;
}

const COLUMNS: readonly DataGridColumn<EntryRow>[] = [
  {
    id: "analyte",
    header: "Analyte",
    cell: (row) => row.analyteName,
    sortValue: (row) => row.analyteName,
  },
  {
    id: "code",
    header: "Class code",
    cell: (row) => row.classCode,
    sortValue: (row) => row.classCode,
    width: 180,
  },
  { id: "label", header: "Class label", cell: (row) => row.classLabel },
];

/**
 * The version shown (REQ-BIL-005): who drafted, approved and confirmed it, when it is in
 * effect, its entries, and the lifecycle actions the engine allows on it.
 */
export function MappingVersion({ versionId }: { versionId: string }) {
  const version = useCommandQuery<BillingClassMappingVersionView>(GET_CLASS_MAPPING_VERSION, {
    input: { versionId },
  });
  return (
    <section aria-label="Version shown" className={styles.stack}>
      <QueryContent query={version} what="class mapping version">
        {(loaded) => <VersionContent version={loaded} />}
      </QueryContent>
    </section>
  );
}

function VersionContent({ version }: { version: BillingClassMappingVersionView }) {
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const names = new Map((analytes.data ?? []).map((analyte) => [analyte.id, analyte.name]));
  // An analyte the catalog does not list shows its ID, never a guessed name.
  const rows = version.entries.map((entry) => ({
    ...entry,
    analyteName: names.get(entry.analyteId) ?? entry.analyteId,
  }));
  return (
    <>
      <div className={mappings.shownHeader}>
        <h2 className={mappings.shownTitle}>Version {version.version}</h2>
        <div className={mappings.shownActions}>
          <MappingActions version={version} />
        </div>
      </div>
      <Section title="Lifecycle">
        <MappingFacts version={version} />
      </Section>
      <Section title="Entries">
        {rows.length === 0 ? (
          <p className={styles.hint}>This version has no entries.</p>
        ) : (
          <DataGrid
            label="Mapping entries"
            columns={COLUMNS}
            rows={rows}
            getRowId={(row) => row.analyteId}
          />
        )}
      </Section>
    </>
  );
}
