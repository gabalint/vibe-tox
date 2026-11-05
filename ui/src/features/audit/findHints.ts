/**
 * What Find matches for a record type, as the engine's owner modules search it
 * (`find_audit_records`): the start of each value, ignoring case. Every type is also found by
 * its whole record ID.
 */
const VALUES: Readonly<Record<string, string>> = {
  instrument: "Instrument code or name",
  app_version: "Version",
  update_approval: "Version",
  user: "Username or name",
  session: "Username or name",
  backup_target: "Target name",
  backup_run: "Day requested (YYYY-MM-DD)",
  billing_handoff: "Handoff number or accession",
  report: "Report number",
  report_revision: "Report number",
  report_export: "Report number",
  order: "Accession or placer order number",
  accession: "Accession number",
  specimen: "Accession number",
  container: "Container label or accession",
  practitioner: "Name or NPI",
  client: "Name or code",
  specimen_type: "Code or name",
  analyte: "Code or name",
  method: "Code or name",
  orderable: "Code or name",
};

/** Find's accessible name and placeholder. */
export interface FindHint {
  label: string;
  placeholder: string;
}

/**
 * Find's wording for the chosen record type, or for every type. Patients are searched by
 * name or identifier only for people who may read them (`patients`).
 */
export function findHint(recordType: string | undefined, patients: boolean): FindHint {
  if (recordType === undefined) {
    return {
      label: "Find a record",
      placeholder: "Accession, number, code, name or record ID",
    };
  }
  const values =
    recordType === "subject"
      ? patients
        ? "Patient name or identifier"
        : undefined
      : VALUES[recordType];
  return values === undefined
    ? { label: "Find by record ID", placeholder: "Record ID" }
    : { label: `Find: ${values} or record ID`, placeholder: `${values} or record ID` };
}
