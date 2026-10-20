// Engine errors (docs/governance/security-audit-logging.md#errors-returned-to-the-ui).
import type { CommandError } from "@/generated/CommandError";
import type { ErrorCode } from "@/generated/ErrorCode";

const FALLBACK_MESSAGES: Record<ErrorCode, string> = {
  validation: "Check the highlighted fields.",
  not_found: "The record was not found.",
  conflict: "Someone else changed this record. Reload it and try again.",
  forbidden: "You do not have permission for this action.",
  unauthenticated: "Sign in to continue.",
  session_locked: "Your session is locked. Sign in to continue.",
  gate_blocked: "This action is blocked until the listed holds are resolved.",
  unavailable: "This function is not available in this version of the app.",
  internal: "The app could not complete the action.",
};

const CODES = new Set<string>(Object.keys(FALLBACK_MESSAGES));

function isCommandError(value: unknown): value is CommandError {
  if (typeof value !== "object" || value === null) return false;
  const candidate: Record<string, unknown> = { ...value };
  return (
    typeof candidate.code === "string" &&
    CODES.has(candidate.code) &&
    typeof candidate.message === "string" &&
    Array.isArray(candidate.fieldErrors) &&
    Array.isArray(candidate.gateReasons) &&
    typeof candidate.correlationId === "string"
  );
}

/**
 * Normalizes anything a command rejects with. Engine errors pass through; anything else
 * (an unknown or unpermitted command, a transport failure) becomes `unavailable` without
 * echoing the raw text, which can carry internal detail.
 */
export function toCommandError(value: unknown): CommandError {
  if (isCommandError(value)) return value;
  return {
    code: "unavailable",
    message: FALLBACK_MESSAGES.unavailable,
    fieldErrors: [],
    gateReasons: [],
    correlationId: "",
  };
}

/** The text to show for an error: the engine's safe message, or a plain default. */
export function toUserMessage(error: CommandError): string {
  return error.message.trim() || FALLBACK_MESSAGES[error.code];
}

/**
 * Where a form's values differ from the command input it builds: an engine path prefix and
 * the form path it maps to, such as `evidence` → `""` when the form holds the evidence
 * fields at its root. Prefixes match whole path segments; the longest one wins.
 */
export type FieldPaths = Readonly<Record<string, string>>;

/**
 * The form field an engine field error belongs to. The engine names list entries
 * `evidence.diagnoses[0].code`; form fields are named `diagnoses.0.code`.
 */
export function toFormField(enginePath: string, fieldPaths: FieldPaths = {}): string {
  const path = enginePath.replace(/\[(\d+)\]/g, ".$1");
  const prefix = Object.keys(fieldPaths)
    .filter((key) => path === key || path.startsWith(`${key}.`))
    .sort((a, b) => b.length - a.length)[0];
  if (prefix === undefined) return path;
  const target = fieldPaths[prefix] ?? "";
  const rest = path.slice(prefix.length + 1);
  return [target, rest].filter(Boolean).join(".");
}

/** Engine field errors keyed by form field name, for `Form`'s `validationErrors`. */
export function toValidationErrors(
  error: CommandError | null,
  fieldPaths?: FieldPaths,
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const fieldError of error?.fieldErrors ?? []) {
    errors[toFormField(fieldError.field, fieldPaths)] = fieldError.message;
  }
  return errors;
}

/**
 * Plain-language names of the engine's gate codes: the one place they are labelled. The
 * engine's message follows each label; a code without a label shows the message alone.
 */
const GATE_LABELS: Record<string, string> = {
  qc_pending: "QC not assessed",
  qc_rejected: "QC rejected",
  qc_stale: "QC changed after its assessment",
  qc_conflict: "Changed QC row awaits a decision",
  prep_pending: "Preparation batch not assessed",
  prep_rejected: "Preparation batch rejected",
  prep_stale: "Preparation batch changed after its assessment",
  prep_not_completed: "Preparation batch not completed",
  prep_no_steps: "No preparation steps recorded",
  execution_failed: "Test execution failed",
  run_closed: "Run closed",
  deviation_open: "Open deviation",
  lot_expired: "Lot expired",
  lot_withdrawn: "Lot withdrawn",
  instrument_not_in_service: "Instrument not in service",
  method_not_current: "Method version not current",
  dilution_not_supported: "Dilution not supported",
  quarantine_open: "Unresolved imported row",
  validity_assessment_missing: "Specimen validity not assessed",
  validity_assessment_stale: "Specimen validity changed",
  validity_assessment_changed: "Specimen validity changed",
  result_revision_not_current: "Result changed",
  result_revision_changed: "Result changed",
  dependency_changed: "Analytical evidence changed",
  // Report release (REQ-RPT-001, REQ-RPT-005; design brief §3.4).
  identity_hold_open: "Open identity hold",
  specimen_not_accepted: "Specimen not accepted",
  specimen_held: "Specimen held",
  specimen_rejected: "Specimen rejected",
  container_quality_hold: "Container quality hold",
  container_excursion_hold: "Container excursion hold",
  order_not_accepted: "Order not accepted",
  clarification_hold_open: "Open clarification hold",
  component_cancelled: "Test cancelled",
  item_not_accepted: "Ordered test not accepted",
  item_in_multiple_accessions: "Ordered test in more than one accession",
  validity_hold: "Specimen validity hold",
  validity_recollect: "Specimen validity requires recollection",
  review_stale: "Technical review out of date",
  self_release: "You reviewed results on this report",
  limit_missing: "Cutoff or limit missing",
  laboratory_profile_missing: "Laboratory profile not set",
  nothing_changed: "Nothing changed since the current revision",
  report_withdrawn: "Report withdrawn",
  candidate_stale: "Report content changed since the candidate was prepared",
  candidate_expired: "Candidate expired",
  candidate_not_ready: "Candidate not ready",
  revision_conflict: "Changed since it was shown",
  artifact_missing: "Issued report file missing or damaged",
  revision_impact_open: "Impact review open",
  destination_not_verified: "Destination not verified",
  destination_retired: "Destination retired",
  destination_reported_wrong: "Destination reported wrong",
  destination_not_of_client: "Destination of another client",
  second_person_required: "Needs a second person",
  nothing_to_report: "No result ready to report",
  stored_content_mismatch: "Stored report content damaged",
};

/**
 * The plain label of a gate code. Codes may carry a cause after a colon, such as
 * `review_stale:qc_rejected`; those take the label of their prefix.
 */
export function gateLabel(code: string): string | undefined {
  const key = Object.hasOwn(GATE_LABELS, code) ? code : (code.split(":")[0] ?? code);
  return Object.hasOwn(GATE_LABELS, key) ? GATE_LABELS[key] : undefined;
}
