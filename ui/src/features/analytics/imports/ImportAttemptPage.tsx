import { useState } from "react";
import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Section } from "@/components/Section";
import { Tabs } from "@/components/Tabs";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { useRecordSequence } from "@/lib/recordSequence";
import { useRouteTabs } from "@/lib/routeTabs";
import { GET_IMPORT_ATTEMPT } from "../contract";
import { IMPORTS_LIST, importAttemptPath } from "../paths";
import { failureText } from "../resultLabels";
import { failureFixFacts } from "./fixLinks";
import { ImportFile } from "./ImportFile";
import { ImportOutcomeBadge } from "./ImportOutcomeBadge";
import { QuarantineItems } from "./QuarantineItems";
import { RetryImportDialog } from "./RetryImportDialog";
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
              <QuarantineItems attempt={loaded} />
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
                  <QuarantineItems attempt={loaded} />
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
    : `Quarantined (${attempt.rowsQuarantined})`;
}

/** The attempt's counts; a file attempt adds the rows it counted or kept without a result. */
function outcomeFacts(attempt: ImportAttemptDetail) {
  const facts = [
    {
      label: attempt.file === null ? "Message failure" : "File failure",
      value: failureText(attempt.failureCode),
    },
    ...failureFixFacts(attempt),
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
  return (
    <>
      <Button onPress={() => setRetrying(true)}>Retry import</Button>
      {isRetrying && (
        <RetryImportDialog receiptId={attempt.receiptId} onClose={() => setRetrying(false)} />
      )}
    </>
  );
}
