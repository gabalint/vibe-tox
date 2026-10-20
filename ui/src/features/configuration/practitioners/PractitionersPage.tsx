import { useState } from "react";
import { Button } from "@/components/Button";
import type { DataGridColumn } from "@/components/DataGrid";
import { DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import type { PractitionerView } from "@/generated/PractitionerView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { DirectoryHistory } from "../clients/DirectoryHistory";
import { LIST_PRACTITIONERS } from "../contract";
import { byId, Directory } from "../Directory";
import { CorrectPractitionerDialog } from "./CorrectPractitionerDialog";
import { RegisterPractitionerDialog } from "./RegisterPractitionerDialog";

function practitionerName({ familyName, givenNames }: PractitionerView): string {
  return givenNames ? `${familyName}, ${givenNames}` : familyName;
}

const COLUMNS: readonly DataGridColumn<PractitionerView>[] = [
  {
    id: "name",
    header: "Name",
    identifier: true,
    cell: practitionerName,
    sortValue: practitionerName,
  },
  {
    id: "state",
    header: "Availability",
    cell: (row) => (row.active === false ? "Retired" : "Active"),
  },
  { id: "npi", header: "NPI", cell: (row) => row.npi, sortValue: (row) => row.npi, width: 160 },
];

const FIND_FIELDS = (row: PractitionerView) => [practitionerName(row), row.npi];

/**
 * The practitioners who order tests (REQ-ORD-001 references), for people with
 * `manage_configuration`. `list_practitioners` returns every practitioner and has no search
 * yet, so the find runs here; the worst case, 10,000, stays within NFR-33.
 */
export function PractitionersPage() {
  const query = useCommandQuery<PractitionerView[]>(LIST_PRACTITIONERS);
  const record = useOpenRecord();
  const [editing, setEditing] = useState(false);
  const canEdit = useHasCapability("manage_configuration");
  return (
    <Directory
      title="Practitioners"
      noun={{ many: "practitioners", one: "practitioner" }}
      query={query}
      columns={COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Name or NPI"
      onRowAction={(row, order) => record.open(row.id, order)}
      openRowId={record.openId}
      onOrderChange={record.followOrder}
      panel={openRecordPanel(record, query.data, byId, (open) => (
        <DetailPanel
          identifier={practitionerName(open)}
          actions={
            canEdit && <Button onPress={() => setEditing(true)}>Correct practitioner</Button>
          }
          onClose={record.close}
          onPrevious={record.previous}
          onNext={record.next}
        >
          <Facts
            items={[
              { label: "Family name", value: open.familyName },
              { label: "Given names", value: open.givenNames ?? "Not provided" },
              { label: "NPI", value: open.npi },
              { label: "Availability", value: open.active === false ? "Retired" : "Active" },
              { label: "Revision", value: open.revision },
            ]}
          />
          <DirectoryHistory key={open.id} record={{ kind: "practitioner", id: open.id }} />
          {editing && (
            <CorrectPractitionerDialog practitioner={open} onClose={() => setEditing(false)} />
          )}
        </DetailPanel>
      ))}
      add={{
        label: "Register practitioner",
        emptyText: "Register the practitioners who order tests.",
        dialog: (onClose) => (
          <RegisterPractitionerDialog
            onSaved={(practitionerId) => record.open(practitionerId)}
            onClose={onClose}
          />
        ),
      }}
    />
  );
}
