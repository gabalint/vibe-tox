import { useFieldArray, useFormContext, useWatch } from "react-hook-form";
import { FieldList } from "@/components/FieldList";
import { CheckList, ChoiceInput } from "@/components/FormFields";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import type { MethodView } from "@/generated/MethodView";
import { useCommandQuery } from "@/lib/commands";
import { GET_METHOD_VERSION, LIST_METHOD_VERSIONS } from "../contract";
import { PURPOSES, versionStatus } from "../labels";
import { codedOptions } from "../options";
import { EMPTY_COMPONENT, type OrderableFormValues } from "./orderableValues";

/** The components of a test or panel version, each with its purpose (REQ-SPC-004). */
export function ComponentFields({ methods }: { methods: readonly MethodView[] }) {
  const { control } = useFormContext<OrderableFormValues>();
  const rows = useFieldArray({ control, name: "components" });
  return (
    <FieldList
      legend="Components"
      emptyText="A test has one component; a panel has at least one."
      items={rows.fields}
      itemLabel={(index) => `Component ${index + 1}`}
      addLabel="Add component"
      onAdd={() => rows.append(EMPTY_COMPONENT)}
      onRemove={rows.remove}
      renderItem={(index) => <ComponentRow index={index} methods={methods} />}
    />
  );
}

function ComponentRow({ index, methods }: { index: number; methods: readonly MethodView[] }) {
  const { setValue } = useFormContext<OrderableFormValues>();
  const [methodId, methodVersionId] = useWatch<
    OrderableFormValues,
    [`components.${number}.methodId`, `components.${number}.methodVersionId`]
  >({ name: [`components.${index}.methodId`, `components.${index}.methodVersionId`] });
  // A changed method or version makes the choices that depend on it stale.
  const clearAnalytes = () => setValue(`components.${index}.analyteIds`, []);
  const clearVersion = () => {
    setValue(`components.${index}.methodVersionId`, null);
    clearAnalytes();
  };
  const versionChoice = { index, onChange: clearAnalytes };
  return (
    <>
      <ChoiceInput
        name={`components.${index}.methodId`}
        label="Method"
        options={codedOptions(methods)}
        onChange={clearVersion}
        isRequired
      />
      {methodId ? (
        <MethodVersions {...versionChoice} methodId={methodId} />
      ) : (
        <MethodVersionChoice {...versionChoice} options={[]} />
      )}
      <ChoiceInput
        name={`components.${index}.purpose`}
        label="Purpose"
        options={PURPOSES}
        isRequired
      />
      <AnalyteChoice index={index} methodVersionId={methodVersionId} />
    </>
  );
}

interface MethodVersionChoiceProps {
  index: number;
  options: readonly { id: string; label: string }[];
  onChange: () => void;
}

function MethodVersionChoice({ index, options, onChange }: MethodVersionChoiceProps) {
  return (
    <ChoiceInput
      name={`components.${index}.methodVersionId`}
      label="Method version"
      options={options}
      description="Choose the current approved version."
      onChange={onChange}
      isRequired
    />
  );
}

/** Every version of the chosen method with its status; the engine accepts only the current one. */
function MethodVersions({
  methodId,
  ...choice
}: Omit<MethodVersionChoiceProps, "options"> & { methodId: string }) {
  const versions = useCommandQuery<CatalogVersionSummary[]>(LIST_METHOD_VERSIONS, {
    input: { methodId },
  });
  const options = (versions.data ?? []).map((version) => ({
    id: version.id,
    label: `Version ${version.version} (${versionStatus(version).label.toLowerCase()})`,
  }));
  return <MethodVersionChoice {...choice} options={options} />;
}

function AnalyteChoice({
  index,
  methodVersionId,
}: {
  index: number;
  methodVersionId: string | null;
}) {
  if (!methodVersionId) return null;
  return <MethodAnalytes index={index} methodVersionId={methodVersionId} />;
}

function MethodAnalytes({ index, methodVersionId }: { index: number; methodVersionId: string }) {
  const method = useCommandQuery<MethodVersionView>(GET_METHOD_VERSION, {
    input: { methodVersionId },
  });
  const options = (method.data?.analytes ?? []).map((analyte) => ({
    id: analyte.analyteId,
    label: `${analyte.analyteCode} ${analyte.analyteName}`,
  }));
  return (
    <CheckList
      name={`components.${index}.analyteIds`}
      legend="Analytes reported"
      options={options}
      requiredMessage="Choose at least one analyte."
    />
  );
}
