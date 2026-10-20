import type { Tone } from "@/components/tones";
import type { CatalogVersionStatus } from "@/generated/CatalogVersionStatus";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { ComponentPurpose } from "@/generated/ComponentPurpose";
import type { MethodCertainty } from "@/generated/MethodCertainty";
import type { OrderableKind } from "@/generated/OrderableKind";
import type { OutputScale } from "@/generated/OutputScale";
import type { ThresholdRounding } from "@/generated/ThresholdRounding";
import type { ThresholdRule } from "@/generated/ThresholdRule";
import type { ValidityAction } from "@/generated/ValidityAction";
import type { Choice } from "@/lib/choices";

export const ORDERABLE_KINDS: readonly Choice<OrderableKind>[] = [
  { id: "test", label: "Test (one component)" },
  { id: "panel", label: "Panel (one or more components)" },
];

export const CERTAINTIES: readonly Choice<MethodCertainty>[] = [
  { id: "presumptive", label: "Presumptive (screening)" },
  { id: "definitive", label: "Definitive (confirmation)" },
];

export const OUTPUT_SCALES: readonly Choice<OutputScale>[] = [
  { id: "qualitative", label: "Qualitative" },
  { id: "quantitative", label: "Quantitative" },
];

export const THRESHOLD_RULES: readonly Choice<ThresholdRule>[] = [
  { id: "at_or_above", label: "Positive at or above the cutoff" },
  { id: "above", label: "Positive only above the cutoff" },
];

export const THRESHOLD_ROUNDINGS: readonly Choice<ThresholdRounding>[] = [
  { id: "before_rounding", label: "Compare the value as reported, before rounding" },
  { id: "after_rounding", label: "Compare after rounding to the display scale" },
];

// REQ-SPC-004: validity components are told apart from drug results by their purpose.
export const PURPOSES: readonly Choice<ComponentPurpose>[] = [
  { id: "analytical", label: "Analytical (drug result)" },
  { id: "validity", label: "Specimen validity" },
];

export const VALIDITY_ACTIONS: readonly Choice<ValidityAction>[] = [
  { id: "none", label: "No action" },
  { id: "qualifier", label: "Report with a qualifier" },
  { id: "hold", label: "Hold the results" },
  { id: "recollect", label: "Request recollection" },
  { id: "comment", label: "Add a comment" },
];

const STATUSES: Record<CatalogVersionStatus, { label: string; tone: Tone }> = {
  draft: { label: "Draft", tone: "warning" },
  approved: { label: "Approved", tone: "neutral" },
  retired: { label: "Retired", tone: "neutral" },
};

/**
 * A version's status in words, with its tone. An approved version whose effective period
 * another approval ended is shown as replaced, as the engine recorded it.
 */
export function versionStatus(version: Pick<CatalogVersionSummary, "status" | "replacedBy">): {
  label: string;
  tone: Tone;
} {
  if (version.status === "approved" && version.replacedBy !== null) {
    return { label: "Replaced", tone: "neutral" };
  }
  return STATUSES[version.status];
}
