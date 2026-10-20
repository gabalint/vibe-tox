import { ChoiceInput } from "@/components/FormFields";
import type { CatalogCodedName } from "@/generated/CatalogCodedName";
import type { CreateOrderable } from "@/generated/CreateOrderable";
import type { OrderableView } from "@/generated/OrderableView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { CodedNameFields, trimCodedName } from "../CodedNameFields";
import { ORDERABLE_COLUMNS, orderablePath } from "../catalogColumns";
import { LIST_ORDERABLES } from "../contract";
import { byId, Directory } from "../Directory";
import { ORDERABLE_KINDS } from "../labels";
import { useOrderableFilters } from "./useOrderableFilters";

const TITLE = "Tests and panels";
const FIND_FIELDS = (row: OrderableView) => [row.code, row.name];

interface OrderableValues extends CatalogCodedName {
  kind: string | null;
}

/**
 * The tests and panels clinics order (REQ-ANA-009), filtered by kind and by the version
 * state the engine reports. A row opens its page with its versions. Worst case about 1,000,
 * so the UI filters the whole list.
 */
export function OrderablesPage() {
  const orderables = useCommandQuery<OrderableView[]>(LIST_ORDERABLES);
  const filters = useOrderableFilters();
  const openPage = useOpenRecordPage(orderablePath, TITLE);
  return (
    <Directory
      title={TITLE}
      noun={{ many: "tests and panels", one: "test or panel" }}
      query={orderables}
      columns={ORDERABLE_COLUMNS}
      getRowId={byId}
      findFields={FIND_FIELDS}
      findPlaceholder="Code or name"
      filters={filters}
      onRowAction={(row, order) => openPage(row.id, order)}
      add={{
        label: "Add test or panel",
        emptyText: "Add a test or panel, then draft its first version.",
        dialog: (onClose) => (
          <CommandDialog<OrderableValues, CreateOrderable, string>
            title="Add test or panel"
            submitLabel="Add test or panel"
            command={{ name: "create_orderable", invalidates: [LIST_ORDERABLES] }}
            defaultValues={{ code: "", name: "", kind: null }}
            toInput={(values) => ({
              ...trimCodedName(values),
              kind: checked(choiceOf(ORDERABLE_KINDS, values.kind)),
            })}
            successTitle={(values) => `${values.code.trim()} added`}
            onDone={(id) => openPage(id, [])}
            errorTitle="Not saved"
            onClose={onClose}
          >
            <CodedNameFields />
            <ChoiceInput name="kind" label="Kind" options={ORDERABLE_KINDS} isRequired />
          </CommandDialog>
        ),
      }}
    />
  );
}
