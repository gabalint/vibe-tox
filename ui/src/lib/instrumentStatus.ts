// An instrument's service status in words (REQ-ANA-007): bench setup records it, and the
// instruments of laboratory setup show it.
import type { Tone } from "@/components/tones";
import type { InstrumentStatus } from "@/generated/InstrumentStatus";

/** `null` means nobody has recorded a status yet; the engine treats that as not ready. */
export function instrumentStatus(status: InstrumentStatus | null): { label: string; tone: Tone } {
  if (status === "in_service") return { label: "In service", tone: "neutral" };
  if (status === "out_of_service") return { label: "Out of service", tone: "danger" };
  return { label: "Status not recorded", tone: "warning" };
}
