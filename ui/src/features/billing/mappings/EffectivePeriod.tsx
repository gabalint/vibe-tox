import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import { useFormatTime } from "@/lib/format";

/** When a version is the current mapping, as the engine recorded it. */
export function EffectivePeriod({
  version,
}: {
  version: Pick<BillingClassMappingVersionSummary, "effectiveFrom" | "effectiveTo">;
}) {
  const formatTime = useFormatTime();
  if (version.effectiveFrom === null) return "Not yet effective";
  const end =
    version.effectiveTo === null ? "until replaced" : `to ${formatTime(version.effectiveTo)}`;
  return `From ${formatTime(version.effectiveFrom)} ${end}`;
}
