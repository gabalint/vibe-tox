import { StatusBadge } from "@/components/StatusBadge";
import type { ImportOutcome } from "@/generated/ImportOutcome";
import { importStatus } from "../resultLabels";

export function ImportOutcomeBadge({ outcome }: { outcome: ImportOutcome }) {
  const status = importStatus(outcome);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}
