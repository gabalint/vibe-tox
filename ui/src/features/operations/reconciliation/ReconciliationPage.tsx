import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { RecoveryReconciliation } from "@/generated/RecoveryReconciliation";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import { AnchorCounts } from "../AnchorCounts";
import { GET_RECOVERY_RECONCILIATION, MANAGE_QUALITY } from "../contract";
import { Person } from "../Person";
import { CloseReconciliationDialog } from "./CloseReconciliationDialog";
import { RaiseFloorsDialog } from "./RaiseFloorsDialog";

/**
 * The recovery reconciliation that every restore opens (REQ-OPS-002, design brief §3.8). Any
 * session reads it; a quality manager closes it with every item attested, and raises the number
 * floors. The engine holds release, delivery, billing handoff and label printing while it is
 * open.
 */
export function ReconciliationPage() {
  const query = useCommandQuery<RecoveryReconciliation | null>(GET_RECOVERY_RECONCILIATION);
  const canManage = useHasCapability(MANAGE_QUALITY);
  const [dialog, setDialog] = useState<"close" | "floors" | null>(null);
  const close = () => setDialog(null);
  const data = query.data;
  return (
    <Page>
      <PageHeader
        title="Recovery reconciliation"
        actions={
          canManage &&
          data && (
            <>
              {!data.closed && (
                <Button variant="primary" onPress={() => setDialog("close")}>
                  Close recovery reconciliation
                </Button>
              )}
              <Button onPress={() => setDialog("floors")}>Raise number floors</Button>
            </>
          )
        }
        description="What the laboratory checks after this installation was restored from a backup."
      />
      <QueryContent query={query} what="recovery reconciliation">
        {(reconciliation) =>
          reconciliation ? (
            <ReconciliationContent reconciliation={reconciliation} />
          ) : (
            <EmptyState
              title="No recovery reconciliation"
              description="This installation has not been restored from a backup, so it has no recovery reconciliation."
            />
          )
        }
      </QueryContent>
      {dialog === "close" && data && (
        <CloseReconciliationDialog reconciliation={data} onClose={close} />
      )}
      {dialog === "floors" && data && <RaiseFloorsDialog floors={data.floors} onClose={close} />}
    </Page>
  );
}

function ReconciliationContent({ reconciliation }: { reconciliation: RecoveryReconciliation }) {
  const formatTime = useFormatTime();
  const { closed } = reconciliation;
  return (
    <div className={styles.stack}>
      {/* While it is open, the frame's banner says what waits; the sentence is shown once. */}
      {closed && (
        <Alert tone="info" title="Recovery reconciliation closed">
          Closed {formatTime(closed.at)} by <Person id={closed.by} />.
        </Alert>
      )}
      <Section title="Restore">
        <Facts
          layout="compact"
          items={[
            { label: "Backup made", value: <Timestamp value={reconciliation.setCreatedAt} /> },
            { label: "Restore applied", value: <Timestamp value={reconciliation.appliedAt} /> },
            {
              label: "Lost interval from",
              value: <Timestamp value={reconciliation.lostIntervalFrom} />,
            },
            { label: "Label jobs held", value: String(reconciliation.heldLabelJobs) },
            { label: "Accession number floor", value: reconciliation.floors.accession },
            { label: "Container code floor", value: reconciliation.floors.container },
          ]}
        />
      </Section>
      <Section title="Audit anchors">
        <AnchorCounts summary={reconciliation.anchors} />
      </Section>
      <Section title="Checklist">
        <ul className={styles.list}>
          {reconciliation.items.map((item) => (
            <li key={item.code}>
              <span className={styles.subheading}>{item.label}</span>
              <p className={styles.hint}>{item.guidance}</p>
            </li>
          ))}
        </ul>
      </Section>
    </div>
  );
}
