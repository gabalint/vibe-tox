import { useNavigate } from "react-router";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import { useCommandQuery } from "@/lib/commands";
import { GET_CLASS_MAPPING_VERSION } from "../contract";
import { classMappingPath } from "../paths";
import { EntriesDialog } from "./EntriesDialog";

interface NewDraftDialogProps {
  /** The current version, whose entries the draft starts from; `null` when there is none. */
  currentVersionId: string | null;
  onClose: () => void;
}

/** Opens the one mapping draft, starting from the current version's entries when one exists. */
export function NewDraftDialog({ currentVersionId, onClose }: NewDraftDialogProps) {
  if (currentVersionId === null) return <DraftForm entries={[]} onClose={onClose} />;
  return <FromCurrent versionId={currentVersionId} onClose={onClose} />;
}

function FromCurrent({ versionId, onClose }: { versionId: string; onClose: () => void }) {
  const current = useCommandQuery<BillingClassMappingVersionView>(GET_CLASS_MAPPING_VERSION, {
    input: { versionId },
  });
  if (current.data) return <DraftForm entries={current.data.entries} onClose={onClose} />;
  return (
    <Dialog
      title="Loading the current mapping"
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      {current.isError ? (
        <ErrorState title="The current mapping could not be loaded" error={current.error} />
      ) : (
        <LoadingState label="Loading the current mapping…" />
      )}
    </Dialog>
  );
}

function DraftForm({
  entries,
  onClose,
}: {
  entries: BillingClassMappingVersionView["entries"];
  onClose: () => void;
}) {
  const navigate = useNavigate();
  return (
    <EntriesDialog
      title="New class mapping draft"
      submitLabel="Save draft"
      command="draft_class_mapping_version"
      entries={entries}
      toInput={(rows) => ({ entries: rows })}
      successTitle="Draft saved"
      onDone={(version) => navigate(classMappingPath(version.version))}
      onClose={onClose}
    />
  );
}
