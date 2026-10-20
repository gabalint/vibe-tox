import { StatusBadge } from "@/components/StatusBadge";

/** The engine's version numbers of a versioned catalog record, from its directory row. */
export interface VersionNumbers {
  versionInEffect: number | null;
  draftVersion: number | null;
}

/**
 * "v3 in effect, v4 awaiting approval", "v1 awaiting approval" or "No version in effect".
 * Both numbers come from the engine; nothing here decides which version is in effect.
 */
export function versionStateText({ versionInEffect, draftVersion }: VersionNumbers): string {
  const inEffect = versionInEffect === null ? null : `v${versionInEffect} in effect`;
  const draft = draftVersion === null ? null : `v${draftVersion} awaiting approval`;
  if (inEffect && draft) return `${inEffect}, ${draft}`;
  return inEffect ?? draft ?? "No version in effect";
}

/** The version state, as a warning only while a draft awaits someone's approval. */
export function VersionState({ record }: { record: VersionNumbers }) {
  return (
    <StatusBadge tone={record.draftVersion === null ? "neutral" : "warning"}>
      {versionStateText(record)}
    </StatusBadge>
  );
}
