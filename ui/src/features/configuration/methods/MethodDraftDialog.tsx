import { useFieldArray, useForm, useFormContext } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { FieldGrid, FieldSection } from "@/components/Form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { CatalogVersionRef } from "@/generated/CatalogVersionRef";
import type { DraftMethodVersion } from "@/generated/DraftMethodVersion";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import type { UpdateMethodDraft } from "@/generated/UpdateMethodDraft";
import { useCommandQuery } from "@/lib/commands";
import { LIST_ANALYTES, LIST_SPECIMEN_TYPES, METHOD_QUERIES } from "../contract";
import { DraftDialog, type DraftMode, draftBase } from "../DraftDialog";
import { DECIMAL_FORMAT, DISPLAY_SCALE_FORMAT } from "../formats";
import { CERTAINTIES, OUTPUT_SCALES, THRESHOLD_ROUNDINGS, THRESHOLD_RULES } from "../labels";
import { codedOptions } from "../options";
import {
  EMPTY_ANALYTE,
  EMPTY_METHOD,
  type MethodFormValues,
  methodValues,
  RANGE_POLICIES,
  toMethodContent,
} from "./methodValues";

interface MethodDraftDialogProps {
  methodId: string;
  mode: DraftMode<MethodVersionView>;
  onSaved: (draft: CatalogVersionRef) => void;
  onClose: () => void;
}

/**
 * Drafts a method version or edits an open draft (REQ-ANA-002): the specimen type, how
 * certain and how scaled its results are, how a cutoff classifies them, and each analyte's
 * unit, cutoff, detection and quantitation limits and reportable range, as exact decimals.
 */
export function MethodDraftDialog({ methodId, mode, onSaved, onClose }: MethodDraftDialogProps) {
  const base = draftBase(mode);
  const form = useForm<MethodFormValues>({
    defaultValues: base ? methodValues(base) : EMPTY_METHOD,
  });
  return (
    <DraftDialog<MethodFormValues, MethodVersionView, DraftMethodVersion, UpdateMethodDraft>
      mode={mode}
      commands={{ draft: "draft_method_version", update: "update_method_draft" }}
      invalidates={METHOD_QUERIES}
      form={form}
      newInput={(values) => ({ methodId, content: toMethodContent(values) })}
      editInput={(draft, values) => ({
        methodVersionId: draft.id,
        expectedRevision: draft.revision,
        content: toMethodContent(values),
      })}
      onSaved={onSaved}
      onClose={onClose}
      size="wide"
    >
      <MethodFields />
    </DraftDialog>
  );
}

function MethodFields() {
  const specimenTypes = useCommandQuery<SpecimenTypeView[]>(LIST_SPECIMEN_TYPES);
  return (
    <>
      <FieldSection title="Method">
        <FieldGrid>
          <ChoiceInput
            name="specimenTypeId"
            label="Specimen type"
            options={codedOptions(specimenTypes.data)}
            isRequired
          />
          <ChoiceInput name="certainty" label="Certainty" options={CERTAINTIES} isRequired />
          <ChoiceInput name="outputScale" label="Result scale" options={OUTPUT_SCALES} isRequired />
          <ChoiceInput
            name="thresholdRule"
            label="Cutoff rule"
            options={THRESHOLD_RULES}
            isRequired
          />
          <ChoiceInput
            name="thresholdRounding"
            label="Cutoff comparison"
            options={THRESHOLD_ROUNDINGS}
            isRequired
          />
          <TextInput
            name="reportNotes"
            label="Report limitations and notes"
            description="Optional laboratory-approved method information for patient reports, up to 4000 characters. No clinical text is supplied automatically."
          />
        </FieldGrid>
      </FieldSection>
      <AnalyteList />
    </>
  );
}

function AnalyteList() {
  const { control } = useFormContext<MethodFormValues>();
  const rows = useFieldArray({ control, name: "analytes" });
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const options = codedOptions(analytes.data);
  return (
    <FieldList
      name="analytes"
      legend="Analytes measured"
      emptyText="Add at least one analyte."
      items={rows.fields}
      itemLabel={(index) => `Analyte ${index + 1}`}
      addLabel="Add analyte"
      onAdd={() => rows.append(EMPTY_ANALYTE)}
      onRemove={rows.remove}
      renderItem={(index) => (
        <>
          <ChoiceInput
            name={`analytes.${index}.analyteId`}
            label="Analyte"
            options={options}
            isRequired
          />
          <TextInput name={`analytes.${index}.unit`} label="Unit" description="Such as ng/mL" />
          <TextInput name={`analytes.${index}.cutoff`} label="Cutoff" format={DECIMAL_FORMAT} />
          <TextInput
            name={`analytes.${index}.limitOfDetection`}
            label="Limit of detection"
            format={DECIMAL_FORMAT}
          />
          <TextInput
            name={`analytes.${index}.limitOfQuantitation`}
            label="Limit of quantitation"
            format={DECIMAL_FORMAT}
          />
          <TextInput
            name={`analytes.${index}.reportableLow`}
            label="Reportable range low"
            format={DECIMAL_FORMAT}
          />
          <TextInput
            name={`analytes.${index}.reportableHigh`}
            label="Reportable range high"
            format={DECIMAL_FORMAT}
          />
          <ChoiceInput
            name={`analytes.${index}.rangePolicy`}
            label="Range policy"
            options={RANGE_POLICIES}
            isRequired
            description="Report only is the legacy default. Require within range blocks review/release unless a quantitative final concentration is within inclusive configured bounds or the result has an explicit invalid, indeterminate or not-performed disposition. No automatic dilution or conversion."
          />
          <TextInput
            name={`analytes.${index}.referenceInterval`}
            label="Reference interval"
            description="Optional laboratory-approved patient-report text, up to 1000 characters. Separate from the analytical reportable range and decision cutoff."
          />
          <TextInput
            name={`analytes.${index}.displayScale`}
            label="Display scale"
            format={DISPLAY_SCALE_FORMAT}
          />
        </>
      )}
    />
  );
}
