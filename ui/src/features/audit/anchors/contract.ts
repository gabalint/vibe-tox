// Audit anchors (NFR-18, ADR 0008; M4 design brief §4 and the IPC contract in
// docs/planning/m4-contract.md). Reading needs `view_audit`; exporting needs `backup_restore`.
// The engine decides lag and findings; this screen shows them as received. Verifying is an
// audited read, and with `chooseFolder` the folder dialog opens in Rust and answers `null` when
// it is cancelled: the UI never sends a path. Both an export with no available target and an
// unreadable chosen folder fail with `conflict`.
import type { CommandName } from "@/generated/CommandName";

export const BACKUP_RESTORE = "backup_restore";

export const LIST_LIMIT = 100;

/** `get_anchor_status() → AnchorStatusView`. */
export const GET_ANCHOR_STATUS = "get_anchor_status" satisfies CommandName;
/** `list_anchor_exports({input: ListAnchorExportsInput}) → AnchorExportView[]`. */
export const LIST_ANCHOR_EXPORTS = "list_anchor_exports" satisfies CommandName;
/** `export_anchors_now() → ExportReport`. */
export const EXPORT_ANCHORS_NOW = "export_anchors_now" satisfies CommandName;
/** `verify_audit_anchors({input: VerifyAuditAnchorsInput}) → AnchorVerification | null`. */
export const VERIFY_AUDIT_ANCHORS = "verify_audit_anchors" satisfies CommandName;

/** What an export makes stale: the anchor screens and the anchor-lag health check. */
export const ANCHOR_QUERIES: readonly CommandName[] = [
  GET_ANCHOR_STATUS,
  LIST_ANCHOR_EXPORTS,
  "get_host_health",
];
