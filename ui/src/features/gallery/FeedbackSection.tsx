import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { LoadingState } from "@/components/LoadingState";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

export function FeedbackSection() {
  return (
    <GallerySection title="Status and feedback">
      <div className={styles.row}>
        <StatusBadge tone="neutral">Approved</StatusBadge>
        <StatusBadge tone="info">Queued</StatusBadge>
        <StatusBadge tone="warning">Draft awaiting approval</StatusBadge>
        <StatusBadge tone="hold">Identity hold</StatusBadge>
        <StatusBadge tone="danger">Label print failed</StatusBadge>
        <StatusBadge tone="success">Restore test passed</StatusBadge>
      </div>
      <div className={styles.stack}>
        <Alert tone="info" title="Report queued">
          The synthetic report is waiting to be rendered.
        </Alert>
        <Alert tone="success" title="Restore test passed">
          The synthetic backup restored and verified.
        </Alert>
        <Alert tone="warning" title="Draft awaiting approval">
          A reviewer must approve the synthetic draft.
        </Alert>
        <Alert tone="hold" title="Identity hold">
          Release is blocked until the synthetic identity mismatch is resolved.
        </Alert>
        <Alert tone="danger" title="Label print failed">
          The synthetic label was not printed.
        </Alert>
        <Facts
          items={[
            { label: "Accession", value: "SYN-000123" },
            { label: "Status", value: <StatusBadge tone="info">Queued</StatusBadge> },
            { label: "Received", value: <Timestamp value="2026-10-12T14:03:05Z" /> },
          ]}
        />
        <LoadingState />
        <EmptyState
          title="No synthetic records"
          description="Records appear here once they are received."
          action={<Button>Refresh</Button>}
        />
        <ErrorState
          error={{
            code: "internal",
            message: "The app could not complete the action.",
            fieldErrors: [],
            gateReasons: [],
            correlationId: "corr-0000-synthetic",
          }}
          onRetry={() => undefined}
        />
      </div>
    </GallerySection>
  );
}
