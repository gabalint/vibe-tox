import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import type { DataGridColumn } from "@/components/DataGrid";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { TextInput } from "@/components/FormFields";
import type { AnalyteHistoryEntry } from "@/generated/AnalyteHistoryEntry";
import type { AnalyteView } from "@/generated/AnalyteView";
import type { CatalogCodedName } from "@/generated/CatalogCodedName";
import type { CreateAnalyte } from "@/generated/CreateAnalyte";
import { CommandDialog } from "@/lib/CommandDialog";
import { optionalText } from "@/lib/choices";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { CodedNameFields, trimCodedName } from "../CodedNameFields";
import { CorrectionHistory } from "../CorrectionHistory";
import { ANALYTE_COLUMNS, notProvided } from "../catalogColumns";
import { LIST_ANALYTES } from "../contract";
import { byId, Directory, type DirectoryFilters, useClearParams } from "../Directory";
import { RETIRED_PARAM, useShowRetired } from "../useShowRetired";
import { CorrectAnalyteDialog } from "./CorrectAnalyteDialog";

const FIND_FIELDS = (row: AnalyteView) => [row.code, row.name, row.loincCode];

const availability = (row: { active: boolean }) => (row.active ? "Active" : "Retired");

const COLUMNS: readonly DataGridColumn<AnalyteView>[] = [
  ...ANALYTE_COLUMNS,
  { id: "state", header: "Availability", cell: availability, width: 120 },
];

const historyFacts = (row: AnalyteHistoryEntry) =>
  [row.code, row.name, row.loincCode ?? "No LOINC", row.drugClassCode ?? "No drug class"]
    .concat(availability(row))
    .join(" · ");

const PARAMS = [RETIRED_PARAM];
const KEEP_ALL = () => true;

function useRetiredFilter(): DirectoryFilters<AnalyteView> {
  const { showRetired, moreFilters, scopeLabel } = useShowRetired("analytes");
  const clear = useClearParams(PARAMS);
  const scope = useMemo(() => (row: AnalyteView) => showRetired || row.active, [showRetired]);
  return {
    controls: null,
    moreFilters,
    scopeLabel,
    isSet: showRetired,
    scope,
    keep: KEEP_ALL,
    clear,
  };
}

interface AnalyteValues extends CatalogCodedName {
  loincCode: string;
  drugClassCode: string;
}

/**
 * The drugs and validity markers methods measure (REQ-ANA-002), found by code, name or
 * LOINC. A row opens the analyte in the side panel (`?open=`). The LOINC and drug class
 * codes show as the laboratory provided them. Worst case about 2,000, so the UI filters.
 * Retired analytes are hidden until "Show retired"; `?open=` still finds one. With
 * `manage_configuration`, the panel corrects or retires the analyte as a new revision.
 */
export function AnalytesPage() {
  const analytes = useCommandQuery<AnalyteView[]>(LIST_ANALYTES);
  const record = useOpenRecord();
  const filters = useRetiredFilter();
  const [editing, setEditing] = useState(false);
  const canEdit = useHasCapability("manage_configuration");
  return (
    <Directory
      title="Analytes"
      noun={{ many: "analytes", one: "analyte" }}
      query={analytes}
      columns={COLUMNS}
      filters={filters}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code, name or LOINC"
      onRowAction={(row, order) => record.open(row.id, order)}
      openRowId={record.openId}
      onOrderChange={record.followOrder}
      panel={openRecordPanel(record, analytes.data, byId, (open) => (
        <DetailPanel
          identifier={open.code}
          actions={canEdit && <Button onPress={() => setEditing(true)}>Correct analyte</Button>}
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
              { label: "Availability", value: availability(open) },
              { label: "Revision", value: open.revision },
            ]}
          />
          <CorrectionHistory<AnalyteHistoryEntry>
            key={open.id}
            command="analyte_history"
            input={{ analyteId: open.id }}
            label="Analyte correction history"
            facts={historyFacts}
          />
          {editing && <CorrectAnalyteDialog analyte={open} onClose={() => setEditing(false)} />}
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
            <CodedNameFields codeDescription="Letters, digits, '.', '_' or '-'; it can be corrected only until something uses the analyte." />
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
