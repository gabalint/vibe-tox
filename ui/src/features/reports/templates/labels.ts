import type { Tone } from "@/components/tones";
import type { LayoutVersionState } from "@/generated/LayoutVersionState";

/** How each layout version state reads and its tone, by meaning. */
export const versionStates: Record<LayoutVersionState, { label: string; tone: Tone }> = {
  draft: { label: "Waiting for approval", tone: "warning" },
  active: { label: "Active", tone: "success" },
  superseded: { label: "Superseded", tone: "neutral" },
  rejected: { label: "Rejected", tone: "danger" },
};
