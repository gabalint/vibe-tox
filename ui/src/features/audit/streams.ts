import type { AuditStream } from "@/generated/AuditStream";

/** The stream filter's value for every stream together, the default view. */
export const ALL_STREAMS = "all";

/**
 * The audit streams, in the order offered, with what the engine records in each: the security
 * stream also holds files saved out of the LIS, the access stream reads of the trail itself.
 */
export const STREAMS: readonly { id: AuditStream; label: string; description: string }[] = [
  {
    id: "security",
    label: "Security",
    description:
      "Sign-ins, users, roles and security settings; backups, restores, updates and exports; instrument and partner connections.",
  },
  {
    id: "clinical",
    label: "Clinical",
    description:
      "Patients, orders, specimens, results, QC, reports and billing, and laboratory setup such as the catalog and the laboratory profile.",
  },
  {
    id: "access",
    label: "Access",
    description:
      "Who opened patient records, orders, specimens, runs, results, reports or billing, and who viewed this audit trail or checked its chain or anchors.",
  },
];

/** What the shown stream or streams hold, for the page description. */
export function streamDescription(stream: AuditStream | undefined): string {
  return (
    STREAMS.find((each) => each.id === stream)?.description ??
    "Security, clinical and access events together."
  );
}

export function isStream(value: string | null): value is AuditStream {
  return STREAMS.some((stream) => stream.id === value);
}

export function streamLabel(stream: AuditStream): string {
  return STREAMS.find((each) => each.id === stream)?.label ?? stream;
}
