import { useState } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { TextInput } from "@/components/FormFields";
import type { ChosenPartnerFolder } from "@/generated/ChosenPartnerFolder";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { TestPartnerFolderInput } from "@/generated/TestPartnerFolderInput";
import { useCommandMutation } from "@/lib/commands";
import row from "../actionRow.module.css";
import { CHOOSE_FOLDER, TEST_FOLDER } from "../contract";
import { problemText } from "../labels";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

/**
 * Section 3, the folder: chosen in the system dialog, never typed, and tested before saving.
 * The engine keeps the chosen folder for this session; the form holds only its handle. Without
 * a new choice, a saved connection's test checks its current folder.
 */
export function FolderFields({ connectionId }: { connectionId: string | null }) {
  const { setValue, control } = useFormContext<ConnectionValues>();
  const folderChoice = useWatch({ control, name: "folderChoice" });
  const [tested, setTested] = useState<FolderTestResult | null>(null);
  const choose = useCommandMutation<ChosenPartnerFolder | null>(CHOOSE_FOLDER, {
    invalidates: [],
  });
  const test = useCommandMutation<FolderTestResult, { input: TestPartnerFolderInput }>(
    TEST_FOLDER,
    { invalidates: [] },
  );
  const pick = () =>
    choose.mutate(
      {},
      {
        onSuccess: (chosen) => {
          if (chosen === null) return;
          setValue("folder", chosen.folder, { shouldDirty: true, shouldValidate: true });
          setValue("folderChoice", chosen.folderChoice, { shouldDirty: true });
          setTested(null);
        },
      },
    );
  const input: TestPartnerFolderInput | null =
    folderChoice !== "" ? { folderChoice } : connectionId !== null ? { connectionId } : null;
  const runTest = () => {
    if (input) test.mutate({ input }, { onSuccess: setTested });
  };
  return (
    <>
      <div className={styles.half}>
        <TextInput
          name="folder"
          label="Output folder"
          description="Files are written as .tmp and renamed when complete; an existing file is never replaced."
          isRequired
          isDisabled
        />
      </div>
      <div className={row.row}>
        <Button isPending={choose.isPending} onPress={pick}>
          Choose folder…
        </Button>
        <Button isDisabled={input === null} isPending={test.isPending} onPress={runTest}>
          Test folder
        </Button>
        {tested && (
          <span role="status" className={tested.problem === null ? row.ok : row.problem}>
            {tested.problem === null
              ? "✓ The LIS can create, write and remove files in this folder."
              : `✗ Not usable: ${problemText(tested.problem)}.`}
          </span>
        )}
      </div>
      {choose.error && <ErrorState title="Folder not chosen" error={choose.error} />}
      {test.error && <ErrorState title="Folder not tested" error={test.error} />}
    </>
  );
}
