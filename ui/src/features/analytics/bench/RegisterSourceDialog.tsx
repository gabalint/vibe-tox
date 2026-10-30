import { useFieldArray, useFormContext } from "react-hook-form";
import { Alert } from "@/components/Alert";
import { Dialog } from "@/components/Dialog";
import { FieldList } from "@/components/FieldList";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { AuConfig } from "@/generated/AuConfig";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { RegisterSourceInput } from "@/generated/RegisterSourceInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, optionalText, YES_NO } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ANALYTES, LIST_INSTRUMENTS, SOURCE_QUERIES } from "../contract";
import { REVISION_INDICATORS, SAMPLE_ID_FIELDS } from "../labels";
import { AU_TEST_CODE, AuSourceFacts } from "./AuSourceFacts";

interface SourceValues {
  senderName: string;
  senderVersion: string;
  sampleIdField: string | null;
  revisionIndicator: string | null;
  reportsFinalConcentration: string | null;
  /** `unit` is asked only for an AU source. */
  testCodes: { sourceTestCode: string; analyteId: string | null; unit: string }[];
}

/**
 * Drafts a result source: the sender and the map from its test codes to analytes
 * (REQ-ANA-003, REQ-INT-001). It imports nothing until someone else approves it. An AU
 * source also restates its analyzer's wire settings and declares each test's unit, since AU
 * messages carry neither a sender identity nor units.
 */
export function RegisterSourceDialog({
  instrument,
  onClose,
}: {
  instrument: InstrumentReadinessView;
  onClose: () => void;
}) {
  const isAu = instrument.transport === "au_tcp" || instrument.transport === "au_serial";
  const instruments = useCommandQuery<InstrumentConfiguration[]>(LIST_INSTRUMENTS, undefined, {
    enabled: isAu,
  });
  if (!isAu) return <SourceDialog instrument={instrument} protocol={null} onClose={onClose} />;
  const connection = instruments.data?.find(
    (item) => item.id === instrument.instrumentId,
  )?.connection;
  if (connection && "au" in connection) {
    return (
      <SourceDialog instrument={instrument} protocol={connection.au.protocol} onClose={onClose} />
    );
  }
  return (
    <Dialog
      title={`Register a result source for ${instrument.name}`}
      isOpen
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose();
      }}
    >
      <QueryContent query={instruments} what="analyzer connection">
        {() => (
          <Alert tone="warning" title="No AU connection">
            This analyzer's connection is no longer AU400 / AU640. Reopen Bench setup and try again.
          </Alert>
        )}
      </QueryContent>
    </Dialog>
  );
}

function SourceDialog({
  instrument,
  protocol,
  onClose,
}: {
  instrument: InstrumentReadinessView;
  /** The analyzer's AU wire settings, or `null` for an ASTM source. */
  protocol: AuConfig | null;
  onClose: () => void;
}) {
  const isAu = protocol !== null;
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
        // AU sources match the container barcode and never mark corrections.
        sampleIdField: isAu
          ? "specimen_id"
          : checked(choiceOf(SAMPLE_ID_FIELDS, values.sampleIdField)),
        revisionIndicator: isAu
          ? "none"
          : checked(choiceOf(REVISION_INDICATORS, values.revisionIndicator)),
        reportsFinalConcentration:
          checked(choiceOf(YES_NO, values.reportsFinalConcentration)) === "yes",
        testCodes: values.testCodes.map((code) => ({
          sourceTestCode: code.sourceTestCode,
          analyteId: checked(code.analyteId),
        })),
        auProfile: protocol
          ? {
              protocol,
              tests: values.testCodes.map((code) => ({
                testCode: code.sourceTestCode,
                unit: optionalText(code.unit),
              })),
            }
          : undefined,
      })}
      // The AU tests follow the test-code rows one for one.
      fieldPaths={{ "auProfile.tests": "testCodes" }}
      successTitle="Source registered; it needs approval before results are imported"
      onClose={onClose}
    >
      {protocol && <AuSourceFacts protocol={protocol} />}
      <TextInput
        name="senderName"
        label="Sender name"
        description={
          isAu
            ? "AU messages carry no sender name: enter the analyzer model you declare, such as AU640."
            : "As the analyzer writes it in the message header."
        }
        isRequired
      />
      <TextInput
        name="senderVersion"
        label="Sender version"
        description={
          isAu ? "The analyzer software version you declare; AU messages carry none." : undefined
        }
        isRequired
      />
      {!isAu && (
        <>
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
        </>
      )}
      <ChoiceInput
        name="reportsFinalConcentration"
        label="Reports final concentration after dilution"
        options={YES_NO}
        isRequired
      />
      <TestCodeList isAu={isAu} />
    </CommandDialog>
  );
}

/** Each source test code and the analyte it reports; codes are kept exactly as typed. */
function TestCodeList({ isAu }: { isAu: boolean }) {
  const { control } = useFormContext<SourceValues>();
  const codes = useFieldArray({ control, name: "testCodes" });
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const options = (analytes.data ?? [])
    .filter((analyte) => analyte.active)
    .map((analyte) => ({
      id: analyte.id,
      label: `${analyte.name} (${analyte.code})`,
    }));
  return (
    <FieldList
      name="testCodes"
      legend="Test codes"
      emptyText="No test codes mapped yet. Map at least one."
      items={codes.fields}
      itemLabel={(index) => `Test code ${index + 1}`}
      addLabel="Add test code"
      onAdd={() => codes.append({ sourceTestCode: "", analyteId: null, unit: "" })}
      onRemove={codes.remove}
      renderItem={(index) => (
        <>
          <TextInput
            name={`testCodes.${index}.sourceTestCode`}
            label="Source test code"
            format={isAu ? AU_TEST_CODE : undefined}
            isRequired
          />
          <ChoiceInput
            name={`testCodes.${index}.analyteId`}
            label="Analyte"
            options={options}
            isRequired
          />
          {isAu && (
            <TextInput
              name={`testCodes.${index}.unit`}
              label="Unit"
              description="The unit the laboratory declares for this assay, such as ng/mL; AU messages carry none. Leave empty for a unitless assay."
            />
          )}
        </>
      )}
    />
  );
}
