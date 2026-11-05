// The M1 audit trail contract (REQ-OPS-001). Both commands need view_audit, and the engine
// records every call as an access event. Types are generated from the engine into
// `@/generated`.

import type { ListAuditEventsInput } from "@/generated/ListAuditEventsInput";

/** The filters of `list_audit_events`, without paging; no stream reads every stream. */
export type AuditFilter = Omit<ListAuditEventsInput, "before" | "limit">;
