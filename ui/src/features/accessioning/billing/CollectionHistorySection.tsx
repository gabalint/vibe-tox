import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import { Timestamp } from "@/components/Timestamp";
import type { CollectionCorrectionView } from "@/generated/CollectionCorrectionView";
import type { CollectionHistory } from "@/generated/CollectionHistory";
import type { CorrectCollectionTimeInput } from "@/generated/CorrectCollectionTimeInput";
import type { ResolveCollectionCorrectionInput } from "@/generated/ResolveCollectionCorrectionInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import styles from "@/styles/layout.module.css";
import {
  type AccessionFormValues,
  collectionTime,
  EMPTY_ACCESSION,
} from "../specimens/accessionValues";
import { CollectionFields } from "../specimens/CollectionFields";
import { ACCESSION_QUERIES } from "../specimens/contract";
import { collectionTimeText } from "../specimens/labels";
import { FINANCIAL_QUERIES } from "./contract";

const INVALIDATES = [...ACCESSION_QUERIES, ...FINANCIAL_QUERIES, "get_collection_history"] as const;

export function CollectionHistorySection({ accessionId }: { accessionId: string }) {
  const [opened, setOpened] = useState(false);
  return opened ? (
    <HistoryQuery accessionId={accessionId} />
  ) : (
    <Button onPress={() => setOpened(true)}>Review collection history</Button>
  );
}

function HistoryQuery({ accessionId }: { accessionId: string }) {
  const query = useCommandQuery<CollectionHistory>("get_collection_history", {
    input: { accessionId },
  });
  return (
    <>
      <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
        Refresh collection history
      </Button>
      <QueryContent query={query} what="collection history">
        {(history) => <CollectionHistoryContent history={history} />}
      </QueryContent>
    </>
  );
}

export function CollectionHistoryContent({ history }: { history: CollectionHistory }) {
  const [dialog, setDialog] = useState<"correct" | CollectionCorrectionView | null>(null);
  const formatTime = useFormatTime();
  const close = () => setDialog(null);
  return (
    <div className={styles.stack}>
      <Facts
        items={[
          { label: "Original collection", value: collectionTimeText(history.original, formatTime) },
          { label: "Current collection", value: collectionTimeText(history.current, formatTime) },
          { label: "Collection revision", value: history.revision },
        ]}
      />
      <p className={styles.hint}>
        A collection correction requires quality review. The original value remains in history.
      </p>
      {history.allowedActions.includes("correct_collection_time") && (
        <Button onPress={() => setDialog("correct")}>Correct collection time</Button>
      )}
      <ol className={styles.list}>
        {history.corrections.map((correction) => (
          <li key={correction.correctionId}>
            <Facts
              items={[
                { label: "Revision", value: correction.revision },
                {
                  label: "Collection time",
                  value: collectionTimeText({ ...history.original, ...correction }, formatTime),
                },
                { label: "Source reference", value: correction.sourceDocumentRef },
                { label: "Reason", value: correction.reason },
                { label: "Recorded", value: <Timestamp value={correction.recordedAt} /> },
                {
                  label: "Quality review",
                  value: correction.resolvedAt
                    ? `Resolved: ${correction.resolutionReason}`
                    : "Pending",
                },
              ]}
            />
            {correction.allowedActions.includes("resolve_collection_correction") && (
              <Button onPress={() => setDialog(correction)}>Resolve collection review</Button>
            )}
          </li>
        ))}
      </ol>
      {dialog === "correct" && <CorrectionDialog history={history} onClose={close} />}
      {dialog && dialog !== "correct" && <ResolveDialog correction={dialog} onClose={close} />}
    </div>
  );
}

function CorrectionDialog({
  history: initialHistory,
  onClose,
}: {
  history: CollectionHistory;
  onClose: () => void;
}) {
  const [history] = useState(initialHistory);
  return (
    <CommandDialog<
      AccessionFormValues & { sourceDocumentRef: string; reason: string },
      CorrectCollectionTimeInput
    >
      title="Correct collection time"
      submitLabel="Record correction"
      command={{ name: "correct_collection_time", invalidates: INVALIDATES }}
      defaultValues={{ ...EMPTY_ACCESSION, sourceDocumentRef: "", reason: "" }}
      toInput={(values) => ({
        accessionId: history.accessionId,
        expectedRevision: history.revision,
        time: collectionTime(values),
        sourceDocumentRef: values.sourceDocumentRef.trim(),
        reason: values.reason.trim(),
      })}
      successTitle="Collection correction recorded; quality review required"
      onClose={onClose}
    >
      <CollectionFields includeCollector={false} />
      <TextInput name="sourceDocumentRef" label="Source document reference" isRequired />
      <TextInput name="reason" label="Correction reason" isRequired />
    </CommandDialog>
  );
}

function ResolveDialog({
  correction,
  onClose,
}: {
  correction: CollectionCorrectionView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ reason: string }, ResolveCollectionCorrectionInput>
      title="Resolve collection review"
      submitLabel="Resolve review"
      command={{ name: "resolve_collection_correction", invalidates: INVALIDATES }}
      defaultValues={{ reason: "" }}
      toInput={(values) => ({
        correctionId: correction.correctionId,
        reason: values.reason.trim(),
      })}
      successTitle="Collection review resolved"
      onClose={onClose}
    >
      <p>
        Record the quality review of the corrected collection information and any effect on testing
        or released reports.
      </p>
      <TextInput name="reason" label="Quality review and resolution reason" isRequired />
    </CommandDialog>
  );
}
