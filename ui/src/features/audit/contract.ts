// The M1 audit trail contract (REQ-OPS-001). Both commands need view_audit, and the engine
// records every call as an access event. Types are generated from the engine into
// `@/generated`.

import type { AuditOutcome } from "@/generated/AuditOutcome";
import type { ListAuditEventsInput } from "@/generated/ListAuditEventsInput";

/**
 * The filters of `list_audit_events` as the page keeps them in its address: no stream reads
 * every stream; days stand for the instants the engine takes, and one "By" value for a kind
 * of principal or a user.
 */
export type AuditFilter = Omit<
  ListAuditEventsInput,
  "before" | "limit" | "recordedFrom" | "recordedBefore" | "byKind" | "byUser"
> & {
  /** The first day shown, `YYYY-MM-DD` in the laboratory's time zone. */
  from?: string;
  /** The last day shown, `YYYY-MM-DD` in the laboratory's time zone. */
  to?: string;
  /** `people`, `system` or a user ID. */
  by?: string;
};

/** Whether any filter but the stream applies. */
export function hasFilters({ stream: _stream, ...rest }: AuditFilter): boolean {
  return Object.values(rest).some(Boolean);
}

export function isOutcome(value: string | null | undefined): value is AuditOutcome {
  return value === "succeeded" || value === "denied" || value === "failed";
}
