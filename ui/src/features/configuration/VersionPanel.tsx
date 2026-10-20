import type { InvokeArgs } from "@tauri-apps/api/core";
import { type ReactNode, useId, useState } from "react";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Toolbar } from "@/components/Toolbar";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { CatalogVersionStatus } from "@/generated/CatalogVersionStatus";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { CommandName } from "@/generated/CommandName";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import styles from "./configuration.module.css";
import { MANAGE_CONFIGURATION } from "./contract";
import type { DraftMode } from "./DraftDialog";
import { VersionActions, type VersionWorkflow } from "./VersionActions";
import { VersionFacts } from "./VersionFacts";

/** Opens the draft dialog of one kind of version; it reports the saved draft. */
type DraftDialogRenderer<View> = (
  mode: DraftMode<View>,
  onSaved: (draft: CatalogVersionRef) => void,
  onClose: () => void,
) => ReactNode;

/** The query that loads one version's content. */
export interface VersionDetailQuery {
  command: CommandName;
  args: (versionId: string) => InvokeArgs;
}

export interface VersionPanelProps<Approve, Retire, View> {
  version: CatalogVersionSummary;
  workflow: VersionWorkflow<Approve, Retire>;
  detail: VersionDetailQuery;
  /** The version's content: facts, then at most one grid, which fills the remaining height. */
  renderContent: (view: View) => ReactNode;
  renderDraftDialog: DraftDialogRenderer<View>;
  /** Whether the record has a draft awaiting approval; the engine allows one open draft. */
  draftOpen: boolean;
  onDraftSaved: (draft: CatalogVersionRef) => void;
}

/**
 * The version shown: its lifecycle actions, facts and content. Catalog versions carry no
 * `allowedActions` yet, so the actions show by capability and the engine's status, as a
 * convenience; the engine decides each one.
 */
export function VersionPanel<Approve, Retire, View extends { status: CatalogVersionStatus }>({
  version,
  workflow,
  detail,
  renderContent,
  renderDraftDialog,
  draftOpen,
  onDraftSaved,
}: VersionPanelProps<Approve, Retire, View>) {
  const headingId = useId();
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const content = useCommandQuery<View>(detail.command, detail.args(version.id));
  const [mode, setMode] = useState<DraftMode<View> | null>(null);
  const view = content.data;
  return (
    <section aria-labelledby={headingId} className={styles.versionContent}>
      <div className={styles.versionHeader}>
        <h2 id={headingId} className={styles.versionHeading}>
          Version {version.version}
        </h2>
        <Toolbar>
          {canManage && view?.status === "draft" && (
            <Button onPress={() => setMode({ kind: "edit", draft: view })}>Edit draft</Button>
          )}
          {canManage && view && !draftOpen && (
            <Button onPress={() => setMode({ kind: "new", base: view })}>
              New draft from this version
            </Button>
          )}
          <VersionActions version={version} workflow={workflow} />
        </Toolbar>
      </div>
      <VersionFacts version={version} />
      {content.isPending && <LoadingState label="Loading the version's content…" rows={4} />}
      {content.isError && (
        <ErrorState
          title="The version's content could not be loaded"
          error={content.error}
          onRetry={() => void content.refetch()}
        />
      )}
      {view && renderContent(view)}
      {mode && renderDraftDialog(mode, onDraftSaved, () => setMode(null))}
    </section>
  );
}
