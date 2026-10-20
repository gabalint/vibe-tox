import { useFieldArray, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { RegisterSourceInput } from "@/generated/RegisterSourceInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, YES_NO } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ANALYTES, SOURCE_QUERIES } from "../contract";
import { REVISION_INDICATORS, SAMPLE_ID_FIELDS } from "../labels";

interface SourceValues {
  senderName: string;
  senderVersion: string;
  sampleIdField: string | null;
  revisionIndicator: string | null;
  reportsFinalConcentration: string | null;
  testCodes: { sourceTestCode: string; analyteId: string | null }[];
}

/**
 * Drafts an ASTM source: the sender the analyzer names in its header and the map from its
 * test codes to analytes (REQ-ANA-003). It imports nothing until someone else approves it.
 */
export function RegisterSourceDialog({
  instrument,
  onClose,
}: {
  instrument: InstrumentReadinessView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<SourceValues, RegisterSourceInput>
      title={`Register a result source for ${instrument.name}`}
      submitLabel="Register source"
      command={{ name: "register_source", invalidates: SOURCE_QUERIES }}
      defaultValues={{
        senderName: "",
        senderVersion: "",
        sampleIdField: null,
        revisionIndicator: null,
        reportsFinalConcentration: null,
        testCodes: [],
      }}
      toInput={(values) => ({
        instrumentId: instrument.instrumentId,
        senderName: values.senderName.trim(),
        senderVersion: values.senderVersion.trim(),
        sampleIdField: checked(choiceOf(SAMPLE_ID_FIELDS, values.sampleIdField)),
        revisionIndicator: checked(choiceOf(REVISION_INDICATORS, values.revisionIndicator)),
        reportsFinalConcentration:
          checked(choiceOf(YES_NO, values.reportsFinalConcentration)) === "yes",
        testCodes: values.testCodes.map((code) => ({
          sourceTestCode: code.sourceTestCode,
          analyteId: checked(code.analyteId),
        })),
      })}
      successTitle="Source registered; it needs approval before results are imported"
      onClose={onClose}
    >
      <TextInput
        name="senderName"
        label="Sender name"
        description="As the analyzer writes it in the message header."
        isRequired
      />
      <TextInput name="senderVersion" label="Sender version" isRequired />
      <ChoiceInput
        name="sampleIdField"
        label="Sample ID is read from"
        options={SAMPLE_ID_FIELDS}
        isRequired
      />
      <ChoiceInput
        name="revisionIndicator"
        label="Corrected results"
        options={REVISION_INDICATORS}
        isRequired
      />
      <ChoiceInput
        name="reportsFinalConcentration"
        label="Reports final concentration after dilution"
        options={YES_NO}
        isRequired
      />
      <TestCodeList />
    </CommandDialog>
  );
}

/** Each source test code and the analyte it reports; codes are kept exactly as typed. */
function TestCodeList() {
  const { control } = useFormContext<SourceValues>();
  const codes = useFieldArray({ control, name: "testCodes" });
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const options = (analytes.data ?? []).map((analyte) => ({
    id: analyte.id,
    label: `${analyte.name} (${analyte.code})`,
  }));
  return (
    <FieldList
      legend="Test codes"
      emptyText="No test codes mapped yet. Map at least one."
      items={codes.fields}
      itemLabel={(index) => `Test code ${index + 1}`}
      addLabel="Add test code"
      onAdd={() => codes.append({ sourceTestCode: "", analyteId: null })}
      onRemove={codes.remove}
      renderItem={(index) => (
        <>
          <TextInput
            name={`testCodes.${index}.sourceTestCode`}
            label="Source test code"
            isRequired
          />
          <ChoiceInput
            name={`testCodes.${index}.analyteId`}
            label="Analyte"
            options={options}
            isRequired
          />
        </>
      )}
    />
  );
}
