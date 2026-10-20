import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { TextInput } from "@/components/FormFields";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { CatalogCodedName } from "@/generated/CatalogCodedName";
import type { CreateAnalyte } from "@/generated/CreateAnalyte";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { CodedNameFields, trimCodedName } from "../CodedNameFields";
import { ANALYTE_COLUMNS, notProvided } from "../catalogColumns";
import { LIST_ANALYTES } from "../contract";
import { byId, Directory } from "../Directory";

const FIND_FIELDS = (row: AnalyteView) => [row.code, row.name, row.loincCode];

interface AnalyteValues extends CatalogCodedName {
  loincCode: string;
  drugClassCode: string;
}

/**
 * The drugs and validity markers methods measure (REQ-ANA-002), found by code, name or
 * LOINC. A row opens the analyte in the side panel (`?open=`). The LOINC and drug class
 * codes show as the laboratory provided them. Worst case about 2,000, so the UI filters.
 */
export function AnalytesPage() {
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const record = useOpenRecord();
  return (
    <Directory
      title="Analytes"
      noun={{ many: "analytes", one: "analyte" }}
      query={analytes}
      columns={ANALYTE_COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code, name or LOINC"
      onRowAction={(row, order) => record.open(row.id, order)}
      openRowId={record.openId}
      onOrderChange={record.followOrder}
      panel={openRecordPanel(record, analytes.data, byId, (open) => (
        <DetailPanel
          identifier={open.code}
          onClose={record.close}
          onPrevious={record.previous}
          onNext={record.next}
        >
          <Facts
            items={[
              { label: "Code", value: open.code },
              { label: "Name", value: open.name },
              { label: "LOINC", value: notProvided(open.loincCode) },
              { label: "Drug class", value: notProvided(open.drugClassCode) },
            ]}
          />
        </DetailPanel>
      ))}
      add={{
        label: "Add analyte",
        emptyText: "Add the drugs and validity markers the methods measure.",
        dialog: (onClose) => (
          <CommandDialog<AnalyteValues, CreateAnalyte, string>
            title="Add analyte"
            submitLabel="Add analyte"
            command={{ name: "create_analyte", invalidates: [LIST_ANALYTES] }}
            defaultValues={{ code: "", name: "", loincCode: "", drugClassCode: "" }}
            toInput={(values) => ({
              ...trimCodedName(values),
              loincCode: optionalText(values.loincCode),
              drugClassCode: optionalText(values.drugClassCode),
            })}
            successTitle={(values) => `Analyte ${values.code.trim()} added`}
            onDone={(id) => record.open(id)}
            errorTitle="Not saved"
            onClose={onClose}
          >
            <CodedNameFields />
            <TextInput name="loincCode" label="LOINC code" description="As provided, if any." />
            <TextInput
              name="drugClassCode"
              label="Drug class code"
              description="As provided, if any."
            />
          </CommandDialog>
        ),
      }}
    />
  );
}
