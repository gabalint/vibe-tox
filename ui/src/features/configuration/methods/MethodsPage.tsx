import type { MethodView } from "@/generated/MethodView";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { AddCodedDialog } from "../AddCodedDialog";
import { METHOD_COLUMNS, methodPath } from "../catalogColumns";
import { LIST_METHODS } from "../contract";
import { byId, Directory } from "../Directory";

const TITLE = "Methods";
const FIND_FIELDS = (row: MethodView) => [row.code, row.name];

/**
 * The methods directory (REQ-ANA-002): each method's version in effect and any draft
 * awaiting approval, from the engine. A row opens the method's page with its versions.
 * Worst case about 200 methods, so the UI filters the whole list.
 */
export function MethodsPage() {
  const methods = useCommandQuery<MethodView[]>(LIST_METHODS);
  const openPage = useOpenRecordPage(methodPath, TITLE);
  return (
    <Directory
      title={TITLE}
      noun={{ many: "methods", one: "method" }}
      query={methods}
      columns={METHOD_COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code or name"
      onRowAction={(row, order) => openPage(row.id, order)}
      add={{
        label: "Add method",
        emptyText: "Add a method, then draft its first version.",
        dialog: (onClose) => (
          <AddCodedDialog
            noun="Method"
            command="create_method"
            invalidates={[LIST_METHODS]}
            onSaved={(id) => openPage(id, [])}
            onClose={onClose}
          />
        ),
      }}
    />
  );
}
