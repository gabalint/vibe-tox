import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import { mappingStatus } from "../labels";
import { EffectivePeriod } from "./EffectivePeriod";

/** Who drafted, approved, confirmed or retired a mapping version and when. */
export function MappingFacts({ version }: { version: BillingClassMappingVersionSummary }) {
  const person = usePersonText();
  const formatTime = useFormatTime();
  const status = mappingStatus(version);
  const event = (at: string | null, by: string | null, none: string, display?: string) =>
    at === null ? none : `${formatTime(at)}${by === null ? "" : ` by ${person(by, display)}`}`;
  const retirement =
    version.retiredAt === null
      ? []
      : [
          {
            label: "Retired",
            value: event(version.retiredAt, version.retiredBy, "", version.retiredByDisplay),
          },
          { label: "Retirement reason", value: version.retireReason ?? "Not recorded" },
        ];
  return (
    <Facts
      layout="compact"
      items={[
        { label: "Status", value: <StatusBadge tone={status.tone}>{status.label}</StatusBadge> },
        {
          label: "Drafted",
          value: event(version.createdAt, version.createdBy, "", version.createdByDisplay),
        },
        {
          label: "Approved",
          value: event(
            version.approvedAt,
            version.approvedBy,
            "Not approved",
            version.approvedByDisplay,
          ),
        },
        {
          label: "Confirmed by the laboratory",
          value: event(
            version.confirmedAt,
            version.confirmedBy,
            "Not confirmed",
            version.confirmedByDisplay,
          ),
        },
        { label: "Effective", value: <EffectivePeriod version={version} /> },
        ...retirement,
      ]}
    />
  );
}
