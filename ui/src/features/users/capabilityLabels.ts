/** Capabilities in laboratory terms, for role descriptions. Unknown codes show as given. */
const CAPABILITY_LABELS: Readonly<Record<string, string>> = {
  accession: "Accession specimens",
  enter_orders: "Enter orders",
  perform_analysis: "Perform analysis",
  review_results: "Review results",
  release_reports: "Release reports",
  manage_quality: "Manage quality control and deviations",
  billing: "Billing",
  view_audit: "View the audit trail",
  export: "Export data",
  backup_restore: "Back up and restore",
  manage_users: "Manage users",
  manage_configuration: "Change configuration",
  approve_configuration: "Approve configuration",
  instrument_intake: "Receive instrument data",
};

export function capabilityLabel(code: string): string {
  return CAPABILITY_LABELS[code] ?? code;
}
