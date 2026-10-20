import { useFieldArray, useFormContext } from "react-hook-form";
import { ErrorState } from "@/components/ErrorState";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { BillingClassMappingEntry } from "@/generated/BillingClassMappingEntry";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import type { CommandName } from "@/generated/CommandName";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ANALYTES, MAPPING_QUERIES } from "../contract";

interface EntryValues {
  analyteId: string | null;
  classCode: string;
  classLabel: string;
}

interface EntriesValues {
  entries: EntryValues[];
}

const EMPTY_ENTRY: EntryValues = { analyteId: null, classCode: "", classLabel: "" };

interface EntriesDialogProps<Input> {
  title: string;
  submitLabel: string;
  /** `draft_class_mapping_version` or `update_class_mapping_draft`. */
  command: CommandName;
  /** The rows the form starts with; one empty row when there are none. */
  entries: readonly BillingClassMappingEntry[];
  toInput: (entries: BillingClassMappingEntry[]) => Input;
  successTitle: string;
  onDone?: (version: BillingClassMappingVersionView) => void;
  onClose: () => void;
}

/**
 * Edits a mapping draft's whole content: each analyte with the billing class code and label
 * the billing owner assigns to it (REQ-BIL-005). Nothing is suggested from panel names or
 * from catalog drug classes; the engine checks every analyte exists.
 */
export function EntriesDialog<Input>({
  entries,
  toInput,
  command,
  ...dialog
}: EntriesDialogProps<Input>) {
  const rows = entries.length === 0 ? [EMPTY_ENTRY] : entries.map((entry) => ({ ...entry }));
  return (
    <CommandDialog<EntriesValues, Input, BillingClassMappingVersionView>
      {...dialog}
      command={{ name: command, invalidates: MAPPING_QUERIES }}
      defaultValues={{ entries: rows }}
      toInput={(values) =>
        toInput(
          values.entries.map((entry) => ({
            analyteId: checked(entry.analyteId),
            classCode: entry.classCode.trim(),
            classLabel: entry.classLabel.trim(),
          })),
        )
      }
    >
      <EntryFields />
    </CommandDialog>
  );
}

function EntryFields() {
  const { control } = useFormContext<EntriesValues>();
  const rows = useFieldArray({ control, name: "entries" });
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const options = (analytes.data ?? []).map((analyte) => ({
    id: analyte.id,
    label: `${analyte.code} ${analyte.name}`,
  }));
  return (
    <>
      {analytes.isError && (
        <ErrorState title="The analytes could not be loaded" error={analytes.error} />
      )}
      <FieldList
        legend="Entries"
        emptyText="Add at least one analyte with its billing class."
        items={rows.fields}
        itemLabel={(index) => `Entry ${index + 1}`}
        addLabel="Add entry"
        onAdd={() => rows.append(EMPTY_ENTRY)}
        onRemove={rows.remove}
        renderItem={(index) => (
          <>
            <ChoiceInput
              name={`entries.${index}.analyteId`}
              label="Analyte"
              options={options}
              isRequired
            />
            <TextInput name={`entries.${index}.classCode`} label="Class code" isRequired />
            <TextInput name={`entries.${index}.classLabel`} label="Class label" isRequired />
          </>
        )}
      />
    </>
  );
}
