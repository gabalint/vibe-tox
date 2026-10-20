// The M1 audit trail contract (REQ-OPS-001). Both commands need view_audit, and the engine
// records every call as an access event. Types are generated from the engine into
// `@/generated`.

import type { ListAuditEventsInput } from "@/generated/ListAuditEventsInput";

/** The filters of `list_audit_events`, without paging. */
export type AuditFilter = Omit<ListAuditEventsInput, "beforeSeq" | "limit">;
