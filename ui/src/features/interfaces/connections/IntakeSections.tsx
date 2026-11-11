import { useState } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ChosenPartnerFolder } from "@/generated/ChosenPartnerFolder";
import type { FolderTestResult } from "@/generated/FolderTestResult";
import type { TestPartnerFolderInput } from "@/generated/TestPartnerFolderInput";
import { useCommandMutation } from "@/lib/commands";
import row from "../actionRow.module.css";
import { CHOOSE_FOLDER, TEST_FOLDER } from "../contract";
import { problemText } from "../labels";
import styles from "./ConnectionDialog.module.css";
import { Box } from "./ConnectionSections";
import type { ConnectionValues } from "./connectionForm";

/** What a refused message becomes: a file moves to Failed; over MLLP the partner reads AE. */
const REFUSED = { file: "Move to Failed and log", mllp: "Reject with AE and log" } as const;

const validSeconds = (value: string) =>
  /^\d+$/.test(value) && Number(value) >= 10 && Number(value) <= 3600
    ? true
    : "Use 10 to 3600 seconds.";

/** How often the input folder is read. */
export function CheckEveryInput() {
  return (
    <TextInput
      name="intake.checkEverySeconds"
      label="Check every"
      description="Seconds, 10 to 3600. Only complete files are read: *.hl7, not *.tmp."
      isRequired
      validate={validSeconds}
    />
  );
}

type FolderField = "folder" | "processedFolder" | "failedFolder";

/** One folder, chosen in the system dialog and never typed; the form keeps its handle. */
function FolderChooser({ name, label }: { name: FolderField; label: string }) {
  const { setValue } = useFormContext<ConnectionValues>();
  const choose = useCommandMutation<ChosenPartnerFolder | null>(CHOOSE_FOLDER, {
    invalidates: [],
  });
  const pick = () =>
    choose.mutate(
      {},
      {
        onSuccess: (chosen) => {
          if (chosen === null) return;
          setValue(name, chosen.folder, { shouldDirty: true, shouldValidate: true });
          setValue(`${name}Choice`, chosen.folderChoice, { shouldDirty: true });
        },
      },
    );
  return (
    <div className={styles.stack}>
      <TextInput name={name} label={label} isRequired isDisabled />
      <div>
        <Button isPending={choose.isPending} onPress={pick}>
          Choose folder…
        </Button>
      </div>
      {choose.error && <ErrorState title="Folder not chosen" error={choose.error} />}
    </div>
  );
}

/**
 * Section 3 for incoming orders from a folder: the input folder the LIS reads, how often, and
 * the Processed and Failed folders read files move to, in year and month subfolders. Test
 * folders checks that the LIS can read the input folder and write to Processed and Failed.
 */
export function IntakeFolderFields({ connectionId }: { connectionId: string | null }) {
  const { control } = useFormContext<ConnectionValues>();
  const [folderChoice, processedFolderChoice, failedFolderChoice] = useWatch({
    control,
    name: ["folderChoice", "processedFolderChoice", "failedFolderChoice"],
  });
  const [tested, setTested] = useState<FolderTestResult | null>(null);
  const test = useCommandMutation<FolderTestResult, { input: TestPartnerFolderInput }>(
    TEST_FOLDER,
    { invalidates: [] },
  );
  const chosen = folderChoice !== "" && processedFolderChoice !== "" && failedFolderChoice !== "";
  const input: TestPartnerFolderInput | null = chosen
    ? { folderChoice, processedFolderChoice, failedFolderChoice }
    : connectionId !== null
      ? { connectionId }
      : null;
  const runTest = () => {
    if (input) test.mutate({ input }, { onSuccess: setTested });
  };
  return (
    <>
      <div className={styles.row}>
        <FolderChooser name="folder" label="Input folder" />
        <FolderChooser name="processedFolder" label="Processed folder" />
        <FolderChooser name="failedFolder" label="Failed folder" />
      </div>
      <div className={styles.half}>
        <CheckEveryInput />
      </div>
      <p className={styles.hint}>
        Read files move to Processed › year › month, or Failed › year › month with the reason in
        Logs; a name already there gets a new one, so nothing is replaced.
      </p>
      <div className={row.row}>
        <Button isDisabled={input === null} isPending={test.isPending} onPress={runTest}>
          Test folders
        </Button>
        {tested && (
          <span role="status" className={tested.problem === null ? row.ok : row.problem}>
            {tested.problem === null
              ? "✓ The LIS can read the input folder and write to Processed and Failed."
              : `✗ Not usable: ${problemText(tested.problem)}.`}
          </span>
        )}
      </div>
      {test.error && <ErrorState title="Folders not tested" error={test.error} />}
    </>
  );
}

/**
 * Section 4 for incoming orders: which messages are accepted and what happens otherwise. A
 * file the LIS refuses moves to Failed; a message received over MLLP is answered AR or AE.
 */
export function AcceptSection() {
  const { control } = useFormContext<ConnectionValues>();
  const mllp = useWatch({ control, name: "protocol" }) === "mllp_listen";
  const refused = mllp ? REFUSED.mllp : REFUSED.file;
  const others = mllp ? "are refused with AR" : "move to Failed";
  return (
    <Box title="4. What to accept">
      <div className={styles.row}>
        <TextInput
          name="intake.acceptFrom"
          label="Accept messages from"
          description={`MSH-4, the sending facility, such as CLINIC-A. Other messages ${others} and the reason is logged.`}
          isRequired
        />
        <ChoiceInput
          name="intake.unknownPatient"
          label="Unknown patient"
          options={[
            { id: "hold", label: "Hold until the patient is registered" },
            { id: "reject", label: refused },
          ]}
          isRequired
        />
        <ChoiceInput
          name="intake.unknownTest"
          label="Unknown test code"
          options={[{ id: "reject", label: refused }]}
        />
      </div>
      <p className={styles.hint}>
        Exact replays are ignored (no second order). Same ID with changed content is held for
        review.
      </p>
    </Box>
  );
}
