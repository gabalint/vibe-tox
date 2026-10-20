import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { Tabs } from "@/components/Tabs";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { RetryImportInput } from "@/generated/RetryImportInput";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { useRecordSequence } from "@/lib/recordSequence";
import { useRouteTabs } from "@/lib/routeTabs";
import { GET_IMPORT_ATTEMPT, IMPORT_QUERIES } from "../contract";
import { IMPORTS_LIST, importAttemptPath } from "../paths";
import { failureText } from "../resultLabels";
import { ImportFile } from "./ImportFile";
import { ImportOutcomeBadge } from "./ImportOutcomeBadge";
import { QuarantineItems } from "./QuarantineItems";
import { SetAsideRows } from "./SetAsideRows";

/**
 * One import attempt (REQ-ANA-003): its counts, the rows held in quarantine with why, and
 * a retry of the whole received message. For a result file it also shows the file, the
 * template version used and, on a second tab, the rows set aside (result files brief §7). Opening it is recorded in the audit trail, since
 * quarantined rows carry sample labels and values. Opened from Result imports, it keeps the
 * list in its breadcrumb with Previous and Next in the list's order.
 */
export function ImportAttemptPage() {
  const { importAttemptId = "" } = useParams();
  const formatTime = useFormatTime();
  const attempt = useCommandQuery<ImportAttemptDetail>(GET_IMPORT_ATTEMPT, {
    input: { importAttemptId },
  });
  const { list, sequence, keep } = useRecordSequence(
    importAttemptId,
    IMPORTS_LIST,
    importAttemptPath,
  );
  const { selectedId: tab, tabs } = useRouteTabs(importAttemptPath(importAttemptId), TABS);
  const detail = attempt.data;
  const title = detail ? `Import attempt ${detail.attemptNo}` : "Import attempt";
  return (
    <Page>
      <PageHeader
        title={title}
        status={detail && <ImportOutcomeBadge outcome={detail.outcome} />}
        description={detail && `Processed ${formatTime(detail.processedAt)}.`}
        breadcrumbs={[list, { label: title, href: importAttemptPath(importAttemptId) }]}
        sequence={sequence}
        actions={
          <>
            <RefreshControl query={attempt} />
            {detail && <RetryImport attempt={detail} />}
          </>
        }
      />
      <QueryContent query={attempt} what="import">
        {(loaded) => (
          <>
            <Section title="Outcome">
              <Facts layout="compact" items={outcomeFacts(loaded)} />
            </Section>
            <ImportFile attempt={loaded} />
            {loaded.file === null ? (
              <QuarantineItems items={loaded.quarantine} />
            ) : (
              <Tabs
                label="Rows of the file"
                tabs={tabs.map((each) => ({ ...each, label: tabLabel(each.id, loaded) }))}
                selectedId={tab}
                routerOptions={keep}
              >
                {tab === "set-aside" ? (
                  <SetAsideRows rows={loaded.setAside} />
                ) : (
                  <QuarantineItems items={loaded.quarantine} />
                )}
              </Tabs>
            )}
          </>
        )}
      </QueryContent>
    </Page>
  );
}

/** A file attempt's rows: those needing a decision, and those listed only (F9, F17). */
const TABS = [
  { id: "quarantine", label: "Quarantined" },
  { id: "set-aside", label: "Set aside" },
] as const;

function tabLabel(id: (typeof TABS)[number]["id"], attempt: ImportAttemptDetail): string {
  return id === "set-aside"
    ? `Set aside (${attempt.setAside.length})`
    : `Quarantined (${attempt.quarantine.length})`;
}

/** The attempt's counts; a file attempt adds the rows it counted or kept without a result. */
function outcomeFacts(attempt: ImportAttemptDetail) {
  const facts = [
    {
      label: attempt.file === null ? "Message failure" : "File failure",
      value: failureText(attempt.failureCode),
    },
    { label: "Rows", value: String(attempt.rowsTotal) },
    { label: "New results", value: String(attempt.rowsNew) },
    { label: "Duplicates", value: String(attempt.rowsDuplicate) },
    { label: "Reprocessed", value: String(attempt.rowsReprocessed) },
    { label: "QC rows", value: String(attempt.rowsQc) },
    { label: "Quarantined", value: String(attempt.rowsQuarantined) },
  ];
  if (attempt.file === null) return facts;
  return [
    ...facts,
    { label: "Not results", value: String(attempt.rowsNotResult) },
    { label: "Set aside", value: String(attempt.rowsSetAside) },
    { label: "Measured, not ordered (kept)", value: String(attempt.rowsRetained) },
  ];
}

/** Retries the whole received message; the attempt carries no `allowedActions` yet. */
function RetryImport({ attempt }: { attempt: ImportAttemptDetail }) {
  const [isRetrying, setRetrying] = useState(false);
  const retry = useCommandMutation<number, { input: RetryImportInput }>("retry_import", {
    invalidates: IMPORT_QUERIES,
  });
  return (
    <>
      <Button onPress={() => setRetrying(true)}>Retry import</Button>
      {isRetrying && (
        <ReasonConfirmDialog<RetryImportInput>
          title="Retry import"
          confirmLabel="Retry import"
          variant="primary"
          command={retry}
          input={(reason) => ({ receiptId: attempt.receiptId, reason })}
          successTitle="Retry queued; it appears as a new import attempt"
          onClose={() => setRetrying(false)}
        >
          <p>
            The received message is processed again as a new attempt. Rows still open in quarantine
            from earlier attempts are replaced by the outcome of the retry.
          </p>
        </ReasonConfirmDialog>
      )}
    </>
  );
}
