import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { AddCodedDialog } from "../AddCodedDialog";
import { SPECIMEN_TYPE_COLUMNS, specimenTypePath } from "../catalogColumns";
import { LIST_SPECIMEN_TYPES } from "../contract";
import { byId, Directory } from "../Directory";

const TITLE = "Specimen types";
const FIND_FIELDS = (row: SpecimenTypeView) => [row.code, row.name];

/**
 * The specimen types the laboratory accepts (REQ-SPC-008), each with its validity policy's
 * version in effect and any draft awaiting approval, from the engine. A row opens the
 * specimen type's page with its validity policy versions. Worst case about 50.
 */
export function SpecimenTypesPage() {
  const specimenTypes = useCommandQuery<SpecimenTypeView[]>(LIST_SPECIMEN_TYPES);
  const openPage = useOpenRecordPage(specimenTypePath, TITLE);
  return (
    <Directory
      title={TITLE}
      noun={{ many: "specimen types", one: "specimen type" }}
      query={specimenTypes}
      columns={SPECIMEN_TYPE_COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code or name"
      onRowAction={(row, order) => openPage(row.id, order)}
      add={{
        label: "Add specimen type",
        emptyText: "Add the specimen types the laboratory accepts.",
        dialog: (onClose) => (
          <AddCodedDialog
            noun="Specimen type"
            command="create_specimen_type"
            invalidates={[LIST_SPECIMEN_TYPES]}
            onSaved={(id) => openPage(id, [])}
            onClose={onClose}
          />
        ),
      }}
    />
  );
}
