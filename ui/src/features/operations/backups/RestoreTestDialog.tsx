import { ChoiceInput } from "@/components/FormFields";
import type { BackupSetView } from "@/generated/BackupSetView";
import type { RequestRestoreTestInput } from "@/generated/RequestRestoreTestInput";
import type { TargetView } from "@/generated/TargetView";
import { CommandDialog } from "@/lib/CommandDialog";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { LIST_BACKUP_SETS, REQUEST_RESTORE_TEST, RESTORE_TEST_QUERIES } from "../contract";
import { backupKind } from "../labels";

interface TestValues {
  setId: string | null;
}

/**
 * Requests a restore test (design brief §3.6): a set from this target is decrypted into a
 * scratch folder and checked. Live data is never touched. Leaving the set empty tests the
 * newest one.
 */
export function RestoreTestDialog({
  target,
  onClose,
}: {
  target: TargetView;
  onClose: () => void;
}) {
  const formatTime = useFormatTime();
  const sets = useCommandQuery<BackupSetView[]>(LIST_BACKUP_SETS, {
    input: { targetId: target.targetId },
  });
  const options = (sets.data ?? []).map((set) => ({
    id: set.setId,
    label: `${formatTime(set.createdAt)}, ${backupKind(set.kind).toLowerCase()}, app ${set.appVersion}`,
  }));
  return (
    <CommandDialog<TestValues, RequestRestoreTestInput>
      title={`Test a restore from ${target.label}`}
      submitLabel="Start restore test"
      command={{ name: REQUEST_RESTORE_TEST, invalidates: RESTORE_TEST_QUERIES }}
      defaultValues={{ setId: null }}
      toInput={({ setId }) => ({ targetId: target.targetId, setId })}
      successTitle="Restore test requested"
      onClose={onClose}
    >
      <p>
        The copy is decrypted into a scratch folder, opened with this version of the app, and every
        audit trail, audit anchor and stored file is checked. Live data is not touched. The result
        appears under Restore tests.
      </p>
      <ChoiceInput
        name="setId"
        label="Backup set"
        options={options}
        description="Leave empty to test the newest set on this target."
      />
    </CommandDialog>
  );
}
