import { useState } from "react";
import { Button } from "@/components/Button";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import type { ApproveClassMappingVersion } from "@/generated/ApproveClassMappingVersion";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { useCommandMutation } from "@/lib/commands";
import { MAPPING_QUERIES } from "../contract";
import { EntriesDialog } from "./EntriesDialog";

type Action = "update" | "approve" | "confirm" | "retire";

/** Views name the actions the engine allows by their command names. */
const COMMANDS: Record<Action, string> = {
  update: "update_class_mapping_draft",
  approve: "approve_class_mapping_version",
  confirm: "confirm_class_mapping_version",
  retire: "retire_class_mapping_version",
};

interface VersionInput {
  versionId: string;
  expectedRevision: number;
}

/**
 * Edit, approve, confirm and retire a mapping version, each shown only when the engine
 * allows it for this version's state. The engine refuses an edit by anyone but the author
 * and an approval by the author, unless the laboratory's separation-of-duties policy allows
 * that approval and the author confirms it (ADR 0015).
 */
export function MappingActions({ version }: { version: BillingClassMappingVersionView }) {
  const [dialog, setDialog] = useState<Action | null>(null);
  const allowed = (action: Action) => version.allowedActions.includes(COMMANDS[action]);
  const close = () => setDialog(null);
  const input: VersionInput = {
    versionId: version.versionId,
    expectedRevision: version.revision,
  };
  return (
    <>
      {allowed("update") && <Button onPress={() => setDialog("update")}>Edit entries</Button>}
      {allowed("approve") && (
        <Button variant="primary" onPress={() => setDialog("approve")}>
          Approve version
        </Button>
      )}
      {allowed("confirm") && (
        <Button variant="primary" onPress={() => setDialog("confirm")}>
          Confirm analyte identities
        </Button>
      )}
      {allowed("retire") && (
        <Button variant="danger" onPress={() => setDialog("retire")}>
          Retire version
        </Button>
      )}
      {dialog === "update" && (
        <EntriesDialog
          title={`Edit entries of version ${version.version}`}
          submitLabel="Save draft"
          command="update_class_mapping_draft"
          entries={version.entries}
          toInput={(entries) => ({ ...input, entries })}
          successTitle="Draft saved"
          onClose={close}
        />
      )}
      {dialog === "approve" && (
        <CommandConfirmDialog<ApproveClassMappingVersion>
          title={`Approve class mapping version ${version.version}?`}
          confirmLabel="Approve version"
          variant="primary"
          command={{ name: "approve_class_mapping_version", invalidates: MAPPING_QUERIES }}
          selfApprovalInput={(selfApproval) => ({ ...input, selfApproval })}
          successTitle={`Version ${version.version} approved`}
          onClose={close}
        >
          <p>
            A billing user other than the author must approve, unless your laboratory's
            separation-of-duties policy allows self-approval. The entries can no longer change, and
            the version is used only once the laboratory confirms its analyte identities.
          </p>
        </CommandConfirmDialog>
      )}
      {dialog === "confirm" && (
        <CommandConfirmDialog<VersionInput>
          title={`Confirm class mapping version ${version.version}?`}
          confirmLabel="Confirm analyte identities"
          variant="primary"
          command={{ name: "confirm_class_mapping_version", invalidates: MAPPING_QUERIES }}
          input={input}
          successTitle={`Version ${version.version} is now current`}
          onClose={close}
        >
          <p>
            The laboratory confirms that each analyte listed is the substance the laboratory tests
            under that name. The version becomes the current mapping from now on, and the previous
            current version ends.
          </p>
        </CommandConfirmDialog>
      )}
      {dialog === "retire" && <RetireDialog version={version} input={input} onClose={close} />}
    </>
  );
}

interface RetireDialogProps {
  version: BillingClassMappingVersionView;
  input: VersionInput;
  onClose: () => void;
}

function RetireDialog({ version, input, onClose }: RetireDialogProps) {
  const retire = useCommandMutation<unknown, { input: VersionInput & { reason: string } }>(
    "retire_class_mapping_version",
    { invalidates: MAPPING_QUERIES },
  );
  return (
    <ReasonConfirmDialog
      title={`Retire class mapping version ${version.version}?`}
      confirmLabel="Retire version"
      variant="danger"
      command={retire}
      input={(reason) => ({ ...input, reason })}
      successTitle={`Version ${version.version} retired`}
      onClose={onClose}
    >
      <p>
        New derivations can no longer use it. Derivations that already used it keep it, and the
        version itself is kept.
      </p>
    </ReasonConfirmDialog>
  );
}
