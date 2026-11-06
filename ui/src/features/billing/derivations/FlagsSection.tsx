import { useMemo } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { BillingFlag } from "@/generated/BillingFlag";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { FLAGS } from "../labels";
import { flagFix, heldMessage } from "./flagFixes";

/** The engine's labels for flag references, keyed by reference ID. */
type ReferenceLabels = Readonly<Record<string, string>>;

/** The engine's label for what the flag is about; `undefined` when it is about nothing named. */
const referenceOf = (flag: BillingFlag, references: ReferenceLabels) =>
  flag.refId === "" ? undefined : (references[flag.refId] ?? flag.refId);

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
    cell: (flag) => referenceOf(flag, references) ?? "None",
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
  {
    id: "fix",
    header: "Fix in",
    // Where a blocking flag is fixed, when that has one clear screen.
    cell: (flag) => {
      const fix = flagFix(flag);
      return fix && <Link to={fix.path}>{fix.place(referenceOf(flag, references))}</Link>;
    },
    width: 260,
  },
];

const NO_REFERENCES: ReferenceLabels = {};

/**
 * What the derivation flagged. A blocking flag holds the derivation until the cause is
 * resolved and it is derived again; it links to where it is fixed when that is one clear
 * screen. Informational flags are for the billing reviewer.
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
          {heldMessage(flags)}
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
