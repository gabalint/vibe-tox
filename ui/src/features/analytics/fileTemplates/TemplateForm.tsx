import { useState } from "react";
import { FormProvider, useForm } from "react-hook-form";
import { Navigate, useNavigate } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Form } from "@/components/Form";
import { useToast } from "@/components/Toast";
import type { FileLayout } from "@/generated/FileLayout";
import type { LayoutSampleOpened } from "@/generated/LayoutSampleOpened";
import type { RegisterFileSourceInput } from "@/generated/RegisterFileSourceInput";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import type { SourceTestCodeInput } from "@/generated/SourceTestCodeInput";
import { useCommandMutation } from "@/lib/commands";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { SOURCE_QUERIES } from "../contract";
import { benchInstrumentPath } from "../paths";
import { CheckSection } from "./CheckSection";
import { ColumnsSection, UnitsSection } from "./ColumnsSection";
import { MappingSteps } from "./MappingSteps";
import { type Mappings, NO_MAPPINGS } from "./mappings";
import { SampleSection } from "./SampleSection";
import { ShapeSection } from "./ShapeSection";
import { SoftwareSection } from "./SoftwareSection";
import {
  EMPTY_TEMPLATE,
  type Found,
  fullLayout,
  TEMPLATE_FIELD_PATHS,
  type TemplateValues,
  testCodes,
  toRegisterInput,
  unitsOf,
} from "./templateValues";
import { useTemplateReading } from "./useTemplateReading";

/** The layout and test codes the values describe, once the sample has been read with them. */
function templateOf(
  values: TemplateValues,
  reading: FileLayout | null,
  found: Found | null,
  mappings: Mappings,
): { layout: FileLayout; testCodes: SourceTestCodeInput[] } | null {
  if (reading === null || found === null) return null;
  return {
    layout: fullLayout(reading, unitsOf(values.layout, reading.header), found, mappings),
    testCodes: testCodes(reading, found, mappings),
  };
}

/**
 * The file template of one result-file instrument, drafted step by step on a sample export
 * (result files brief §7; REQ-ANA-003, REQ-INT-009). Saving records a draft source that
 * imports nothing until someone else approves it. Leaving with input asks first.
 */
export function TemplateForm({ instrumentId }: { instrumentId: string }) {
  const navigate = useNavigate();
  const toast = useToast();
  const form = useForm<TemplateValues>({ defaultValues: EMPTY_TEMPLATE });
  const values = form.watch();
  const [sample, setSample] = useState<LayoutSampleOpened | null>(null);
  const [mappings, setMappings] = useState<Mappings>(NO_MAPPINGS);
  const [incomplete, setIncomplete] = useState(false);
  const [saved, setSaved] = useState(false);
  const read = useTemplateReading(sample, values.layout);
  const register = useCommandMutation<SourceRegistrationView, { input: RegisterFileSourceInput }>(
    "register_file_source",
    { invalidates: SOURCE_QUERIES },
  );
  const template = templateOf(values, read.reading, read.found, mappings);
  const onSubmit = form.handleSubmit((submitted) => {
    const draft = templateOf(submitted, read.reading, read.found, mappings);
    setIncomplete(draft === null);
    if (draft === null) return;
    const input = toRegisterInput(instrumentId, submitted, draft.layout, draft.testCodes);
    register.mutate(
      { input },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title: "File template saved; it needs approval" });
          setSaved(true);
        },
      },
    );
  });
  const openSample = (opened: LayoutSampleOpened) => {
    setSample(opened);
    if (opened.bomEncoding) form.setValue("layout.encoding", opened.bomEncoding);
  };
  const toInstrument = () => navigate(benchInstrumentPath(instrumentId));
  return (
    <FormProvider {...form}>
      {/* Before the redirect, so the guard is released when the instrument opens. */}
      <UnsavedChangesGuard when={(form.formState.isDirty || sample !== null) && !saved} />
      {saved && <Navigate to={benchInstrumentPath(instrumentId)} />}
      <Form
        aria-label="New file template"
        onSubmit={onSubmit}
        error={register.error}
        errorTitle="The template was not saved"
        fieldPaths={TEMPLATE_FIELD_PATHS}
        actions={
          <>
            <Button type="submit" variant="primary" isPending={register.isPending}>
              Save draft
            </Button>
            <Button isDisabled={register.isPending} onPress={toInstrument}>
              Cancel
            </Button>
          </>
        }
      >
        <SoftwareSection />
        <SampleSection
          sample={sample}
          onOpened={openSample}
          preview={read.preview}
          header={read.header}
        />
        {read.header && <ShapeSection header={read.header} columns={read.columns} />}
        {read.header && <ColumnsSection columns={read.columns} />}
        {read.reading && (
          <MappingSteps
            reading={read.reading}
            query={read.readingQuery}
            found={read.found}
            mappings={mappings}
            onChange={setMappings}
          />
        )}
        {read.header && <UnitsSection columns={read.columns} />}
        {sample && template && (
          <CheckSection input={{ sampleHandle: sample.sampleHandle, ...template }} />
        )}
        {incomplete && (
          <Alert tone="warning" title="The template is not complete">
            Open a sample export and make every choice down to the columns before saving.
          </Alert>
        )}
      </Form>
    </FormProvider>
  );
}
