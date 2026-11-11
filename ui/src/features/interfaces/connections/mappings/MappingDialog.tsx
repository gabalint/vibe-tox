import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, RecordPickerInput, TextInput } from "@/components/FormFields";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { DraftPartnerMapping } from "@/generated/DraftPartnerMapping";
import type { MappingKind } from "@/generated/MappingKind";
import type { MappingRow } from "@/generated/MappingRow";
import type { OrderableView } from "@/generated/OrderableView";
import { CommandDialog } from "@/lib/CommandDialog";
import { useCommandQuery } from "@/lib/commands";
import { DRAFT_MAPPING, MAPPING_QUERIES } from "../../contract";

interface MappingValuesForm {
  kind: MappingKind;
  lisId: string | null;
  partnerCode: string;
  loinc: string;
  ucumUnit: string;
  sequenceRole: string;
}

const KIND_OPTIONS = [
  { id: "test", label: "Test (OBR-4)" },
  { id: "analyte", label: "Analyte (OBX-3)" },
];

const ROLE_OPTIONS = [
  { id: "first_line", label: "First line" },
  { id: "confirmation", label: "Confirmation" },
];

/** Engine field paths `mapping.x` are form fields `x`. */
const FIELD_PATHS = { mapping: "" };

/**
 * Adds or changes the partner's code for one LIS test or analyte, as a draft the next
 * approval applies (REQ-INT-004). LOINC left empty uses the catalog's; a unit belongs to an
 * analyte, and a quantity in another unit is held. For incoming `orders`, the clinic's order
 * code maps to a LIS test with the sequence role the partner's contract gives it.
 */
export function MappingDialog({
  connectionId,
  orders,
  row,
  onClose,
}: {
  connectionId: string;
  orders: boolean;
  /** The test or analyte to change, or `null` to choose one. */
  row: MappingRow | null;
  onClose: () => void;
}) {
  const values = row?.draft?.values ?? row?.approved ?? null;
  return (
    <CommandDialog<MappingValuesForm, DraftPartnerMapping>
      title={row ? `Map ${row.lisName}` : "Add mapping"}
      submitLabel="Save draft"
      command={{ name: DRAFT_MAPPING, invalidates: MAPPING_QUERIES }}
      defaultValues={{
        kind: row?.kind ?? (orders ? "test" : "analyte"),
        lisId: row?.lisId ?? null,
        partnerCode: values?.partnerCode ?? "",
        loinc: values?.loinc ?? "",
        ucumUnit: values?.ucumUnit ?? "",
        sequenceRole: values?.sequenceRole ?? "",
      }}
      toInput={(form) => ({
        connectionId,
        kind: form.kind,
        lisId: form.lisId ?? "",
        mapping: {
          partnerCode: form.partnerCode.trim(),
          ...(form.loinc.trim() && { loinc: form.loinc.trim() }),
          ...(form.kind === "analyte" &&
            form.ucumUnit.trim() && { ucumUnit: form.ucumUnit.trim() }),
          ...(orders && form.sequenceRole && { sequenceRole: form.sequenceRole }),
        },
      })}
      successTitle="Mapping saved as a draft; it applies once approved"
      errorTitle="Not saved"
      fieldPaths={FIELD_PATHS}
      onClose={onClose}
    >
      {orders ? <OrderCodeFields fixed={row !== null} /> : <MappingFields fixed={row !== null} />}
    </CommandDialog>
  );
}

/** An incoming order code: the LIS test it asks for and that test's sequence role. */
function OrderCodeFields({ fixed }: { fixed: boolean }) {
  const orderables = useCommandQuery<OrderableView[]>("list_orderables", {});
  const options = (orderables.data ?? []).map((record) => ({
    id: record.id,
    label: record.name,
    description: record.code,
  }));
  return (
    <>
      <TextInput
        name="partnerCode"
        label="Partner code"
        description="The order code the clinic's EMR sends in OBR-4, up to 50 characters"
        isRequired
      />
      <RecordPickerInput
        name="lisId"
        label="LIS test"
        options={options}
        isLoading={orderables.isPending}
        isDisabled={fixed}
        isRequired
      />
      <ChoiceInput
        name="sequenceRole"
        label="Sequence role"
        options={ROLE_OPTIONS}
        description="As the partner's contract defines the order code; it is never inferred"
        isRequired
      />
    </>
  );
}

function MappingFields({ fixed }: { fixed: boolean }) {
  const { control, setValue } = useFormContext<MappingValuesForm>();
  const kind = useWatch({ control, name: "kind" });
  const orderables = useCommandQuery<OrderableView[]>("list_orderables", {});
  const analytes = useCommandQuery<AnalyteView[]>("list_analytes", {});
  const records = kind === "test" ? orderables.data : analytes.data;
  const options = (records ?? []).map((record) => ({
    id: record.id,
    label: record.name,
    description: record.code,
  }));
  return (
    <>
      <ChoiceInput
        name="kind"
        label="Maps"
        options={KIND_OPTIONS}
        isRequired
        isDisabled={fixed}
        onChange={() => setValue("lisId", null)}
      />
      <RecordPickerInput
        name="lisId"
        label={kind === "test" ? "LIS test" : "LIS analyte"}
        options={options}
        isLoading={orderables.isPending || analytes.isPending}
        isDisabled={fixed}
        isRequired
      />
      <TextInput
        name="partnerCode"
        label="Partner code"
        description="The code the partner's system uses, up to 50 characters"
        isRequired
      />
      <TextInput
        name="loinc"
        label="LOINC"
        description="Such as 12345-6; empty uses the catalog's LOINC code, if any"
      />
      {kind === "analyte" && (
        <TextInput
          name="ucumUnit"
          label="Unit (UCUM)"
          description="Such as ng/mL; a quantity in another unit is held, never converted"
        />
      )}
    </>
  );
}
