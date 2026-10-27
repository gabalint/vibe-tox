// Report templates (REQ-RPT-007): the laboratory's choice of report layout. The engine lists the
// ready-made layouts, the one in force and the draft waiting for approval, and names what the
// signed-in person may do (`allowedActions`). Reading and previewing need
// `manage_configuration` or `manage_quality`; drafting needs `manage_configuration`; approving
// and rejecting need `manage_quality`. Previews use built-in synthetic data only.
import type { CommandName } from "@/generated/CommandName";

/** `get_report_layouts() → ReportLayoutsView`. */
export const GET_REPORT_LAYOUTS = "get_report_layouts" satisfies CommandName;
/** `preview_report_layout({input:{layout}}) → null`: opens a report window. */
export const PREVIEW_REPORT_LAYOUT = "preview_report_layout" satisfies CommandName;
/** `draft_report_layout({input:{layout}}) → ReportLayoutsView`. */
export const DRAFT_REPORT_LAYOUT = "draft_report_layout" satisfies CommandName;
/** `approve_report_layout({input:{versionId, expectedRevision, selfApproval}}) → ReportLayoutsView`. */
export const APPROVE_REPORT_LAYOUT = "approve_report_layout" satisfies CommandName;
/** `reject_report_layout({input:{versionId, expectedRevision, reason}}) → ReportLayoutsView`. */
export const REJECT_REPORT_LAYOUT = "reject_report_layout" satisfies CommandName;

export const LAYOUT_QUERIES: readonly CommandName[] = [GET_REPORT_LAYOUTS];
