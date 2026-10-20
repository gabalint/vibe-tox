import { useMemo } from "react";
import { Alert } from "@/components/Alert";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { BillingFlag } from "@/generated/BillingFlag";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { FLAGS } from "../labels";

/** The engine's labels for flag references, keyed by reference ID. */
type ReferenceLabels = Readonly<Record<string, string>>;

const columns = (references: ReferenceLabels): DataGridColumn<BillingFlag>[] => [
  {
    id: "flag",
    header: "Flag",
    cell: (flag) => labelOf(FLAGS, flag.code),
    sortValue: (flag) => labelOf(FLAGS, flag.code),
  },
  {
    id: "ref",
    header: "Reference",
    // The engine's label for what the flag is about; the ID when it sent none.
    cell: (flag) => (flag.refId === "" ? "None" : (references[flag.refId] ?? flag.refId)),
    width: 280,
  },
  {
    id: "effect",
    header: "Effect",
    cell: (flag) =>
      flag.blocking ? (
        <StatusBadge tone="hold">Blocking: holds the derivation</StatusBadge>
      ) : (
        <StatusBadge tone="info">For information</StatusBadge>
      ),
    sortValue: (flag) => (flag.blocking ? 0 : 1),
    width: 260,
  },
];

const NO_REFERENCES: ReferenceLabels = {};

/**
 * What the derivation flagged. A blocking flag holds the derivation until the cause is
 * resolved and it is derived again; informational flags are for the billing reviewer.
 */
export function FlagsSection({
  flags,
  references = NO_REFERENCES,
}: {
  flags: readonly BillingFlag[];
  references?: ReferenceLabels;
}) {
  const flagColumns = useMemo(() => columns(references), [references]);
  const blocking = flags.filter((flag) => flag.blocking).length;
  return (
    <Section title="Flags">
      {blocking > 0 && (
        <Alert tone="hold" title="Held">
          {`${blocking} blocking ${blocking === 1 ? "flag holds" : "flags hold"} this derivation. Resolve the cause, then derive again.`}
        </Alert>
      )}
      {flags.length === 0 ? (
        <p className={styles.hint}>No flags.</p>
      ) : (
        <DataGrid
          label="Flags"
          columns={flagColumns}
          rows={flags}
          getRowId={(flag) => `${flag.code}|${flag.refId}`}
        />
      )}
    </Section>
  );
}
