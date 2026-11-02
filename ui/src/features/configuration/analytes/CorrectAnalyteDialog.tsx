import { Alert } from "@/components/Alert";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { AnalyteUsage } from "@/generated/AnalyteUsage";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { UpdateAnalyte } from "@/generated/UpdateAnalyte";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { DIRECTORY_STATES } from "../clients/CorrectClientDialog";
import { GET_METHOD_VERSION, LIST_ANALYTES } from "../contract";

const ANALYTE_USAGE = "analyte_usage";

interface Values {
  code: string;
  name: string;
  loincCode: string;
  drugClassCode: string;
  state: string | null;
  reason: string;
}

/** What a correction of this analyte affects, most specific first. */
function notes(analyte: AnalyteView, usage: AnalyteUsage): string[] {
  const methods = usage.methods
    .map((use) => `${use.methodCode} version ${use.version} (${use.status})`)
    .join(", ");
  return [
    usage.codeInUse
      ? `Code ${analyte.code} is permanent because records use it. To fix a wrong code, retire the analyte and add a new one.`
      : "The code can change only while nothing uses the analyte.",
    usage.methods.length > 0
      ? `Used by ${methods}. To retire it, first remove it from these methods in a new method version.`
      : "Retiring hides it from new methods, test-code maps and file templates; existing records keep it.",
    "Methods and tests show the corrected values. Results, reports and catalog previews already issued keep what they showed.",
  ];
}

/**
 * Corrects or retires the analyte as a new revision with a reason (REQ-ANA-002). The engine
 * says whether the code is still changeable and which current or draft method versions use
 * the analyte; it refuses retirement while any does.
 */
export function CorrectAnalyteDialog({
  analyte,
  onClose,
}: {
  analyte: AnalyteView;
  onClose: () => void;
}) {
  const usage = useCommandQuery<AnalyteUsage>(ANALYTE_USAGE, {
    input: { analyteId: analyte.id },
  });
  return (
    <QueryContent query={usage} what="analyte usage">
      {(used) => (
        <CommandDialog<Values, UpdateAnalyte, AnalyteView>
          title="Correct analyte"
          submitLabel="Save correction"
          command={{
            name: "update_analyte",
            invalidates: [LIST_ANALYTES, "analyte_history", ANALYTE_USAGE, GET_METHOD_VERSION],
          }}
          defaultValues={{
            code: analyte.code,
            name: analyte.name,
            loincCode: analyte.loincCode ?? "",
            drugClassCode: analyte.drugClassCode ?? "",
            state: analyte.active ? "active" : "retired",
            reason: "",
          }}
          toInput={(values) => ({
            analyteId: analyte.id,
            expectedRevision: analyte.revision,
            code: used.codeInUse ? analyte.code : values.code.trim(),
            name: values.name.trim(),
            loincCode: optionalText(values.loincCode),
            drugClassCode: optionalText(values.drugClassCode),
            active: values.state === "active",
            reason: values.reason.trim(),
          })}
          fieldPaths={{ active: "state" }}
          successTitle="Analyte correction saved"
          onClose={onClose}
        >
          <Alert tone="info" compact>
            <ul>
              {notes(analyte, used).map((note) => (
                <li key={note}>{note}</li>
              ))}
            </ul>
          </Alert>
          <TextInput
            name="code"
            label="Code"
            isRequired
            isDisabled={used.codeInUse}
            description={used.codeInUse ? undefined : "Letters, digits, '.', '_' or '-'."}
          />
          <TextInput name="name" label="Name" isRequired />
          <TextInput name="loincCode" label="LOINC code" description="As provided, if any." />
          <TextInput
            name="drugClassCode"
            label="Drug class code"
            description="As provided, if any."
          />
          <ChoiceInput
            name="state"
            label="Availability for new use"
            isRequired
            options={DIRECTORY_STATES}
          />
          <TextInput
            name="reason"
            label="Reason"
            isRequired
            description="Why the analyte is corrected or retired, up to 1000 characters."
          />
        </CommandDialog>
      )}
    </QueryContent>
  );
}
