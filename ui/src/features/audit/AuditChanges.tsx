import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { AuditChangeStatus } from "@/generated/AuditChangeStatus";
import type { AuditEventChanges } from "@/generated/AuditEventChanges";
import type { AuditEventView } from "@/generated/AuditEventView";
import type { GetAuditEventChangesInput } from "@/generated/GetAuditEventChangesInput";
import { useCommandQuery } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import styles from "./audit.module.css";
import { NONE } from "./auditColumns";

/** The sentence under Changes for each status; a list of changed fields needs none. */
const STATUS_TEXT: Readonly<Record<AuditChangeStatus, string | undefined>> = {
  changed: undefined,
  unchanged_revision:
    "This revision changed no field value: it was saved with the same values and a reason.",
  hidden:
    "These are patient values, which your roles cannot read. The changed fields are listed without their values.",
  values_not_kept:
    "The values of this kind of record are not kept in a history, so the old and new values cannot be shown.",
  no_change: "No data changed.",
};

/** The capabilities that show hidden patient values, as their role names say them. */
const CAPABILITY_NAMES: Readonly<Record<string, string>> = {
  enter_orders: "Enter orders",
  manage_quality: "Manage quality",
  billing: "Billing",
};

/**
 * What the event changed (REQ-OPS-001): each changed field's old and new value, read by the
 * engine from the record's own history when the event is opened. Patient values come only to
 * people whose roles may read the patient, and the engine records that they were viewed.
 */
export function AuditChanges({ event }: { event: AuditEventView }) {
  const input: GetAuditEventChangesInput = { stream: event.stream, seq: event.seq };
  const changes = useCommandQuery<AuditEventChanges>("get_audit_event_changes", { input });
  return (
    <Section title="Changes" variant="plain">
      <QueryContent query={changes} what="changes">
        {(data) => <ChangesBody changes={data} />}
      </QueryContent>
    </Section>
  );
}

function ChangesBody({ changes }: { changes: AuditEventChanges }) {
  const hidden = changes.status === "hidden";
  const patient = changes.patient;
  const statusText = STATUS_TEXT[changes.status];
  const needs = changes.readCapabilities.map((code) => CAPABILITY_NAMES[code] ?? code);
  // A recorded detail that Changes already shows, such as "Enabled", is not repeated.
  const shown = new Set(changes.fields.map(({ field }) => field.toLowerCase()));
  const facts = changes.facts.filter(([label]) => !shown.has(label.toLowerCase()));
  const rows: ChangeRow[] = changes.fields.map(({ field, old, new: next }) => ({
    label: field,
    old: old ?? NONE,
    next: next ?? NONE,
  }));
  if (changes.newRevision !== null) {
    rows.unshift({
      label: "Revision",
      old: String(changes.oldRevision ?? NONE),
      next: String(changes.newRevision),
    });
  }
  return (
    <>
      {patient && (
        <Facts
          items={[
            { label: "Patient", value: patient.name },
            { label: "Identifiers", value: patient.identifiers.join("; ") || NONE },
          ]}
        />
      )}
      {statusText && (
        <p className={styles.summary}>
          {statusText}
          {needs.length > 0 && ` Values need the ${needs.join(" or ")} role capability.`}
        </p>
      )}
      {rows.length > 0 && <ChangeTable rows={rows} hidden={hidden} />}
      {facts.length > 0 && (
        <>
          <h3 className={layout.subheading}>Recorded details</h3>
          <Facts items={facts.map(([label, value]) => ({ label, value }))} />
        </>
      )}
    </>
  );
}

interface ChangeRow {
  label: string;
  old: string;
  next: string;
}

/** Each changed field with its old and new value side by side; hidden values say so once. */
function ChangeTable({ rows, hidden }: { rows: readonly ChangeRow[]; hidden: boolean }) {
  return (
    <table className={styles.changes}>
      <thead>
        <tr>
          <th scope="col">Field</th>
          <th scope="col">Old</th>
          <th scope="col">New</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.label}>
            <th scope="row">{row.label}</th>
            {hidden && row.label !== "Revision" ? (
              <td colSpan={2}>Hidden</td>
            ) : (
              <>
                <td>{row.old}</td>
                <td>{row.next}</td>
              </>
            )}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
