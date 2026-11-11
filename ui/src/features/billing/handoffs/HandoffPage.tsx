import { useParams } from "react-router";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { useToast } from "@/components/Toast";
import type { ExportedHandoffFile } from "@/generated/ExportedHandoffFile";
import type { HandoffView } from "@/generated/HandoffView";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { GET_BILLING_HANDOFF } from "../contract";
import { handoffPath } from "../paths";
import { HandoffItems } from "./HandoffItems";

/** Where the breadcrumb leads when the handoff was not opened from a list. */
const HOME = { label: "Billing handoffs", href: "/billing/handoffs" };

/**
 * One billing handoff file (REQ-BIL-002): its digest, its items, and the outcome the billing
 * partner gave for each. Opening it is recorded in the audit trail, since the file carries
 * patient identity.
 */
export function HandoffPage() {
  const { handoffId = "" } = useParams();
  const handoff = useCommandQuery<HandoffView>(GET_BILLING_HANDOFF, { input: { handoffId } });
  const toast = useToast();
  const exportFile = useCommandMutation<ExportedHandoffFile, { input: { handoffId: string } }>(
    "export_billing_handoff",
    { invalidates: [] },
  );
  const run = () =>
    exportFile.mutate(
      { input: { handoffId } },
      {
        onSuccess: (file) =>
          toast.show({
            tone: "success",
            title: `Saved ${file.fileName} to your Downloads folder.`,
          }),
      },
    );
  const formatTime = useFormatTime();
  const person = usePersonText();
  const { list, sequence } = useRecordSequence(handoffId, HOME, handoffPath);
  const data = handoff.data;
  const title = data ? `Billing handoff ${data.handoffNo}` : "Billing handoff";
  // One header in every state, so Previous and Next keep focus while the next one loads.
  return (
    <Page>
      <PageHeader
        title={title}
        breadcrumbs={[list, { label: title, href: handoffPath(handoffId) }]}
        sequence={sequence}
        actions={
          data?.allowedActions.includes("export_billing_handoff") && (
            <Button variant="primary" isPending={exportFile.isPending} onPress={run}>
              Export file
            </Button>
          )
        }
        description={
          data &&
          `Created ${formatTime(data.createdAt)} by ${person(data.createdBy, data.createdByDisplay)}.`
        }
      />
      {exportFile.error && <ErrorState title="Not exported" error={exportFile.error} />}
      <QueryContent query={handoff} what="handoff">
        {(loaded) => (
          // The items fill the rest of the page, beside the open item's panel.
          <>
            <FileSection handoff={loaded} />
            <HandoffItems items={loaded.items} />
          </>
        )}
      </QueryContent>
    </Page>
  );
}

/** Stored file provenance; the export action belongs to the record heading. */
function FileSection({ handoff }: { handoff: HandoffView }) {
  return (
    <Section title="File">
      <Facts
        layout="compact"
        items={[
          { label: "Format", value: handoff.format },
          {
            label: "SHA-256",
            value: <span className={styles.code}>{handoff.artifactSha256}</span>,
          },
        ]}
      />
    </Section>
  );
}
