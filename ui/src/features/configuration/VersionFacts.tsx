import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import { versionStatus } from "./labels";

/** A version's status: draft, approved, replaced or retired. */
export function VersionStatusBadge({ version }: { version: CatalogVersionSummary }) {
  const status = versionStatus(version);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}

/** Who drafted, approved or retired a version and when, and its effective period. */
export function VersionFacts({ version }: { version: CatalogVersionSummary }) {
  const person = usePersonText();
  const formatTime = useFormatTime();
  return (
    <Facts
      layout="compact"
      items={[
        { label: "Status", value: <VersionStatusBadge version={version} /> },
        {
          label: "Drafted",
          value: `${formatTime(version.createdAt)} by ${person(version.createdBy, version.createdByDisplay)}`,
        },
        {
          label: "Approved by",
          value: version.approvedBy
            ? person(version.approvedBy, version.approvedByDisplay)
            : "Not approved",
        },
        { label: "Effective", value: effectiveText(version, formatTime) },
        ...retiredFacts(version, person, formatTime),
      ]}
    />
  );
}

/** The version's effective period, from the engine's times. */
export function effectiveText(
  { effectiveFrom, effectiveTo }: Pick<CatalogVersionSummary, "effectiveFrom" | "effectiveTo">,
  formatTime: (timestamp: string) => string,
): string {
  if (effectiveFrom === null) return "Not yet effective";
  const end = effectiveTo === null ? "until replaced or retired" : `to ${formatTime(effectiveTo)}`;
  return `From ${formatTime(effectiveFrom)} ${end}`;
}

function retiredFacts(
  version: CatalogVersionSummary,
  person: (userId: string) => string,
  formatTime: (timestamp: string) => string,
) {
  if (version.retiredAt === null) return [];
  const by = version.retiredBy ? ` by ${person(version.retiredBy)}` : "";
  return [
    { label: "Retired", value: `${formatTime(version.retiredAt)}${by}` },
    { label: "Retirement reason", value: version.retireReason ?? "Not recorded" },
  ];
}
