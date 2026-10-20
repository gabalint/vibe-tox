import type { AuditStream } from "@/generated/AuditStream";

/** The audit streams, in the order offered; security events are the default view. */
export const STREAMS: readonly { id: AuditStream; label: string }[] = [
  { id: "security", label: "Security" },
  { id: "clinical", label: "Clinical" },
  { id: "access", label: "Access" },
];

export function isStream(value: string | null): value is AuditStream {
  return STREAMS.some((stream) => stream.id === value);
}

export function streamLabel(stream: AuditStream): string {
  return STREAMS.find((each) => each.id === stream)?.label ?? stream;
}
