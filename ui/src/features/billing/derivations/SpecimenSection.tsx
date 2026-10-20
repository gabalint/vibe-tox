import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { DerivationClassView } from "@/generated/DerivationClassView";
import type { DerivationComponentView } from "@/generated/DerivationComponentView";
import type { DerivationSpecimenView } from "@/generated/DerivationSpecimenView";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { CERTAINTIES, DISPOSITIONS, yesNo } from "../labels";

const CLASS_COLUMNS: readonly DataGridColumn<DerivationClassView>[] = [
  {
    id: "code",
    header: "Class code",
    cell: (row) => row.classCode,
    sortValue: (row) => row.classCode,
    width: 140,
  },
  { id: "label", header: "Class label", cell: (row) => row.classLabel },
  {
    id: "certainty",
    header: "Certainty",
    cell: (row) => labelOf(CERTAINTIES, row.certainty),
    width: 200,
  },
  { id: "ordered", header: "Ordered", cell: (row) => yesNo(row.ordered), width: 100 },
  { id: "performed", header: "Performed", cell: (row) => yesNo(row.performed), width: 110 },
  { id: "reported", header: "Reported", cell: (row) => yesNo(row.reported), width: 100 },
];

const COMPONENT_COLUMNS: readonly DataGridColumn<DerivationComponentView>[] = [
  {
    id: "orderable",
    header: "Orderable",
    cell: (row) => row.orderable,
    sortValue: (row) => row.orderable,
  },
  {
    id: "disposition",
    header: "Disposition",
    cell: (row) => labelOf(DISPOSITIONS, row.disposition),
    width: 240,
  },
];

/** One specimen's lineage, billing classes and component dispositions. */
export function SpecimenSection({ specimen }: { specimen: DerivationSpecimenView }) {
  return (
    <Section title={`Accession ${specimen.accessionNumber}`}>
      <div className={styles.stack}>
        <Facts
          items={[
            {
              label: "Order",
              value: <Link to={`/billing/orders/${specimen.orderId}`}>Review order billing</Link>,
            },
            { label: "Order version", value: String(specimen.orderVersion) },
          ]}
        />
        {specimen.classes.length === 0 ? (
          <p className={styles.hint}>No billing classes for this specimen.</p>
        ) : (
          <DataGrid
            label={`Billing classes for accession ${specimen.accessionNumber}`}
            columns={CLASS_COLUMNS}
            rows={specimen.classes}
            getRowId={(row) => `${row.classCode}|${row.certainty}`}
          />
        )}
        <DataGrid
          label={`Components for accession ${specimen.accessionNumber}`}
          columns={COMPONENT_COLUMNS}
          rows={specimen.components}
          getRowId={(row) => row.componentId}
        />
      </div>
    </Section>
  );
}

import { Link } from "react-router";
