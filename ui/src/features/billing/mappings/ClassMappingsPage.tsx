import { useState } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { BILLING, LIST_CLASS_MAPPING_VERSIONS } from "../contract";
import { mappingStatus } from "../labels";
import { MappingVersion } from "./MappingVersion";
import { MappingVersions } from "./MappingVersions";
import { NewDraftDialog } from "./NewDraftDialog";

type Version = BillingClassMappingVersionSummary;

/** The URL parameter holding the shown version's number, such as `?version=3`. */
const VERSION_PARAM = "version";

/** The version in effect, from the engine's status and dates as `mappingStatus` reads them. */
const inEffect = (versions: readonly Version[]) =>
  versions.find((version) => mappingStatus(version).current);

/** Versions waiting for an approval or the laboratory's confirmation, newest first. */
const awaiting = (versions: readonly Version[]) =>
  versions
    .filter((version) => mappingStatus(version).awaiting !== null)
    .sort((a, b) => b.version - a.version);

/**
 * The version named in the URL; without one, the version awaiting approval or confirmation,
 * otherwise the version in effect, otherwise the newest. A number the list does not hold
 * shows nothing rather than another version.
 */
function shownVersion(versions: readonly Version[], param: string | null): Version | undefined {
  if (param !== null) return versions.find((version) => String(version.version) === param);
  const newest = [...versions].sort((a, b) => b.version - a.version)[0];
  return awaiting(versions)[0] ?? inEffect(versions) ?? newest;
}

/** "Version 3 in effect, version 4 awaiting approval", or what exists of it. */
function MappingState({ versions }: { versions: readonly Version[] }) {
  const current = inEffect(versions);
  const parts = [
    current ? `Version ${current.version} in effect` : "No version in effect",
    ...awaiting(versions).map(
      (version) => `version ${version.version} ${mappingStatus(version).awaiting}`,
    ),
  ];
  const isAwaiting = parts.length > 1;
  return <StatusBadge tone={isAwaiting ? "warning" : "neutral"}>{parts.join(", ")}</StatusBadge>;
}

/**
 * The billing class mapping (REQ-BIL-005), one versioned record: which billing class each
 * analyte counts toward. The header says which version is in effect and which awaits approval
 * or confirmation; the compact Versions list above the version shown moves between versions,
 * and the version shown is in the URL (`?version=`). A version is usable once a billing user other
 * than its author approves it (or its author, where the laboratory's separation-of-duties policy
 * allows it) and the laboratory confirms the analyte identities. No mapping ships with the
 * product.
 */
export function ClassMappingsPage() {
  const versions = useCommandQuery<Version[]>(LIST_CLASS_MAPPING_VERSIONS);
  const [params] = useSearchParams();
  const canDraft = useHasCapability(BILLING);
  const [isDrafting, setDrafting] = useState(false);
  const rows = versions.data;
  const shown = rows && shownVersion(rows, params.get(VERSION_PARAM));
  return (
    <Page>
      <PageHeader
        title="Billing class mappings"
        description="The billing class each analyte counts toward, set by the billing owner and confirmed by the laboratory."
        status={rows && rows.length > 0 && <MappingState versions={rows} />}
        actions={
          canDraft && (
            <Button variant="primary" isDisabled={!rows} onPress={() => setDrafting(true)}>
              New draft
            </Button>
          )
        }
      />
      {isDrafting && (
        <NewDraftDialog
          currentVersionId={(rows && inEffect(rows)?.versionId) ?? null}
          onClose={() => setDrafting(false)}
        />
      )}
      <QueryContent
        query={versions}
        what="class mappings"
        loadingRows={6}
        empty={{
          when: (data) => data.length === 0,
          title: "No class mapping yet",
          description: "Start a draft with the analytes your laboratory bills for.",
        }}
      >
        {(data) => (
          <div className={styles.stack}>
            <Section title="Versions">
              <MappingVersions versions={data} shownId={shown?.versionId} />
            </Section>
            {shown ? (
              <MappingVersion key={shown.versionId} versionId={shown.versionId} />
            ) : (
              <EmptyState
                title={`No version ${params.get(VERSION_PARAM) ?? ""}`}
                description="The class mapping has no version with this number. Choose one under Versions."
              />
            )}
          </div>
        )}
      </QueryContent>
    </Page>
  );
}
