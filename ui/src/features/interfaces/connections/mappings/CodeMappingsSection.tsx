import { useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ErrorState } from "@/components/ErrorState";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { DiscardPartnerMappingDraft } from "@/generated/DiscardPartnerMappingDraft";
import type { DraftPartnerMapping } from "@/generated/DraftPartnerMapping";
import type { MappingRow } from "@/generated/MappingRow";
import type { MappingValues } from "@/generated/MappingValues";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerMappings } from "@/generated/PartnerMappings";
import { allowedRowActions } from "@/lib/allowedActions";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import row from "../../actionRow.module.css";
import {
  APPROVE_MAPPINGS,
  DISCARD_MAPPING_DRAFT,
  DRAFT_MAPPING,
  LIST_MAPPINGS,
  MAPPING_QUERIES,
} from "../../contract";
import type { Labelled } from "../../labels";
import styles from "../ConnectionPage.module.css";
import { ApproveMappingsDialog } from "./ApproveMappingsDialog";
import { MappingDialog } from "./MappingDialog";

const ACTIONS = [
  { id: "change", label: "Change" },
  { id: "remove", label: "Remove", variant: "danger" as const },
  { id: "discard", label: "Discard draft" },
];

/** What a row shows: the draft, when there is one, else what is approved. */
const shown = (mapping: MappingRow): MappingValues | null =>
  mapping.draft ? mapping.draft.values : mapping.approved;

function statusOf(mapping: MappingRow, version: number | null): Labelled {
  if (mapping.draft) {
    return mapping.draft.values
      ? { label: "Draft – awaiting approval", tone: "warning" }
      : { label: "Draft: remove – awaiting approval", tone: "warning" };
  }
  if (mapping.approved) return { label: `Approved v${version}`, tone: "success" };
  return { label: "Not mapped – held", tone: "warning" };
}

/** The table; rows open only where they can be changed, so only there the name looks like a link. */
function columns(version: number | null, opens: boolean): readonly DataGridColumn<MappingRow>[] {
  const code = (text: string | undefined | null) => (
    <span className={styles.code}>{text || "—"}</span>
  );
  return [
    {
      id: "lis",
      header: "LIS test / analyte",
      cell: (mapping) => mapping.lisName,
      identifier: opens,
    },
    { id: "partner", header: "Partner code", cell: (mapping) => code(shown(mapping)?.partnerCode) },
    {
      id: "loinc",
      header: "LOINC",
      cell: (mapping) => {
        const values = shown(mapping);
        if (values?.loinc) return code(values.loinc);
        return values && mapping.kind === "analyte" ? "from catalog" : "—";
      },
    },
    { id: "unit", header: "Unit (UCUM)", cell: (mapping) => code(shown(mapping)?.ucumUnit) },
    {
      id: "status",
      header: "Status",
      cell: (mapping) => {
        const status = statusOf(mapping, version);
        return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
      },
      wrap: true,
    },
  ];
}

/**
 * An HL7 connection's code mappings, as in the mockup (REQ-INT-004): each LIS test and
 * analyte with the partner's code, LOINC and UCUM unit and its status. Changes are drafts
 * until a second person approves them as a new version; a result without an approved mapping
 * is held, never sent with a guessed code. `readOnly` for the Overview card.
 */
export function CodeMappingsSection({
  connection,
  readOnly = false,
}: {
  connection: PartnerConnection;
  readOnly?: boolean;
}) {
  const [editing, setEditing] = useState<MappingRow | "new" | null>(null);
  const [approving, setApproving] = useState(false);
  const mappings = useCommandQuery<PartnerMappings>(LIST_MAPPINGS, {
    input: { connectionId: connection.id },
  });
  const draft = useCommandMutation<PartnerMappings, { input: DraftPartnerMapping }>(DRAFT_MAPPING, {
    invalidates: MAPPING_QUERIES,
  });
  const discard = useCommandMutation<PartnerMappings, { input: DiscardPartnerMappingDraft }>(
    DISCARD_MAPPING_DRAFT,
    { invalidates: MAPPING_QUERIES },
  );
  const rowActions = allowedRowActions(ACTIONS, {
    label: (mapping: MappingRow) => mapping.lisName,
    allowed: (mapping) => [
      "change",
      ...(mapping.approved && !mapping.draft ? ["remove"] : []),
      ...(mapping.draft ? ["discard"] : []),
    ],
    onAction: (id, mapping) => {
      const target = { connectionId: connection.id, kind: mapping.kind, lisId: mapping.lisId };
      if (id === "change") setEditing(mapping);
      if (id === "remove") draft.mutate({ input: { ...target, mapping: null } });
      if (id === "discard") discard.mutate({ input: target });
    },
  });
  const rowError = draft.error ?? discard.error;
  const data = mappings.data;
  const canApprove = data?.allowedActions.includes(APPROVE_MAPPINGS) ?? false;
  return (
    <Section title={`Code mappings (LIS → ${connection.settings.partner})`}>
      {!readOnly && (
        <div className={row.row}>
          <Button onPress={() => setEditing("new")}>Add mapping</Button>
          {canApprove && (
            <Button variant="primary" onPress={() => setApproving(true)}>
              Approve mappings
            </Button>
          )}
        </div>
      )}
      {rowError && <ErrorState title="Not saved" error={rowError} />}
      <QueryContent
        query={mappings}
        what="code mappings"
        loadingRows={3}
        empty={{
          when: (loaded) => loaded.rows.length === 0,
          title: "No code mappings",
          description:
            "Add the partner's code for each LIS test and analyte this connection sends.",
        }}
      >
        {(loaded) => (
          <DataGrid
            label="Code mappings"
            columns={columns(loaded.version, !readOnly)}
            rows={loaded.rows}
            getRowId={(mapping) => `${mapping.kind}:${mapping.lisId}`}
            {...(!readOnly && { onRowAction: setEditing, rowActions })}
          />
        )}
      </QueryContent>
      <p className={styles.hint}>
        Unmapped results are held, never sent with a guessed code. A mapping change applies to new
        messages only.
      </p>
      {editing !== null && (
        <MappingDialog
          connectionId={connection.id}
          row={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
        />
      )}
      {approving && data && (
        <ApproveMappingsDialog
          connectionId={connection.id}
          expectedRevision={connection.revision}
          next={(data.version ?? 0) + 1}
          onClose={() => setApproving(false)}
        />
      )}
    </Section>
  );
}
