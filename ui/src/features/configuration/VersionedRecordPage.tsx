import type { UseQueryResult } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { CatalogVersionStatus } from "@/generated/CatalogVersionStatus";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { CommandError } from "@/generated/CommandError";
import { useHasCapability } from "@/lib/commands";
import { type ListCrumb, useRecordSequence } from "@/lib/recordSequence";
import { useSearchParam } from "@/lib/searchParam";
import styles from "./configuration.module.css";
import { MANAGE_CONFIGURATION } from "./contract";
import { VersionList } from "./VersionList";
import { VersionPanel, type VersionPanelProps } from "./VersionPanel";
import { type VersionNumbers, VersionState } from "./VersionState";

/** A versioned catalog record's directory row: its identity and the engine's version numbers. */
interface CatalogRecord extends VersionNumbers {
  id: string;
  code: string;
  name: string;
}

/** The URL parameter holding the shown version's number, such as `?version=3`. */
const VERSION_PARAM = "version";

type VersionedRecordPageProps<Approve, Retire, View> = Omit<
  VersionPanelProps<Approve, Retire, View>,
  "version" | "onDraftSaved" | "draftOpen"
> & {
  recordId: string;
  /** The record's directory query, whose row gives the header and the default version. */
  records: UseQueryResult<readonly CatalogRecord[], CommandError>;
  /** The record's own directory, for the breadcrumb when the page was not opened from a list. */
  home: ListCrumb;
  recordPath: (id: string) => string;
  /** The `h1` while the record loads or cannot be found, such as "Method". */
  genericTitle: string;
  description?: string | undefined;
  versions: UseQueryResult<CatalogVersionSummary[], CommandError>;
};

/**
 * One versioned catalog record (REQ-ANA-002): its header with the version in effect and any
 * draft awaiting approval, the compact Versions list, and the shown version's lifecycle and
 * content. The shown version is in the URL as its number (`?version=3`, readable and stable
 * within the record); without one the page shows the draft awaiting approval, otherwise the
 * version in effect, both as the engine reports them, otherwise the newest. Drafts are
 * written with `manage_configuration` and approved by someone else with
 * `approve_configuration`; earlier versions stay readable after they are replaced.
 */
export function VersionedRecordPage<Approve, Retire, View extends { status: CatalogVersionStatus }>(
  props: VersionedRecordPageProps<Approve, Retire, View>,
) {
  const { recordId, records, home, recordPath, genericTitle, versions } = props;
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const { list, sequence } = useRecordSequence(recordId, home, recordPath);
  const [versionParam, setVersionParam] = useSearchParam(VERSION_PARAM);
  const [drafting, setDrafting] = useState(false);
  const record = records.data?.find((each) => each.id === recordId);
  // The default waits for the directory row, so the page does not show one version, then another.
  const shown =
    versions.data && !records.isPending
      ? shownVersion(versions.data, versionParam, record)
      : undefined;
  const showVersion = (version: number) => setVersionParam(String(version));
  const showDraft = (draft: CatalogVersionRef) => showVersion(draft.version);
  return (
    <Page>
      <PageHeader
        title={record ? `${record.code} ${record.name}` : genericTitle}
        description={props.description}
        breadcrumbs={[list, { label: record?.code ?? genericTitle, href: recordPath(recordId) }]}
        status={record && <VersionState record={record} />}
        sequence={sequence}
        actions={
          canManage &&
          !records.isPending && (
            <DraftAction
              draftVersion={record?.draftVersion ?? null}
              shownVersion={shown?.version}
              onNew={() => setDrafting(true)}
              onShow={showVersion}
            />
          )
        }
      />
      <div className={styles.versionLayout}>
        <VersionList versions={versions} shownId={shown?.id} />
        {shown && (
          <VersionPanel
            key={shown.id}
            version={shown}
            workflow={props.workflow}
            detail={props.detail}
            renderContent={props.renderContent}
            renderDraftDialog={props.renderDraftDialog}
            draftOpen={record?.draftVersion != null}
            onDraftSaved={showDraft}
          />
        )}
      </div>
      {drafting &&
        props.renderDraftDialog({ kind: "new", base: null }, showDraft, () => setDrafting(false))}
    </Page>
  );
}

interface DraftActionProps {
  draftVersion: number | null;
  shownVersion: number | undefined;
  onNew: () => void;
  onShow: (version: number) => void;
}

/**
 * "New draft version", or while a draft awaits approval the way to that draft instead: the
 * engine allows one open draft per record (`draftVersion` on the directory row), and the draft
 * is edited from its own version.
 */
function DraftAction({ draftVersion, shownVersion, onNew, onShow }: DraftActionProps) {
  if (draftVersion === null) {
    return (
      <Button variant="primary" onPress={onNew}>
        New draft version
      </Button>
    );
  }
  if (draftVersion === shownVersion) return null;
  return (
    <Button variant="primary" onPress={() => onShow(draftVersion)}>
      Open draft version {draftVersion}
    </Button>
  );
}

/** The version named in the URL, else the draft, else the version in effect, else the newest. */
function shownVersion(
  versions: readonly CatalogVersionSummary[],
  param: string | null,
  record: VersionNumbers | undefined,
): CatalogVersionSummary | undefined {
  const numbered = (number: number | null | undefined) =>
    number === null || number === undefined
      ? undefined
      : versions.find((version) => version.version === number);
  return (
    numbered(param === null ? null : Number(param)) ??
    numbered(record?.draftVersion) ??
    numbered(record?.versionInEffect) ??
    versions.reduce<CatalogVersionSummary | undefined>(
      (newest, version) => (newest && newest.version > version.version ? newest : version),
      undefined,
    )
  );
}
