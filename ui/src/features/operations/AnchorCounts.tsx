import { Facts } from "@/components/Facts";
import type { AnchorSummary } from "@/generated/AnchorSummary";
import { ANCHOR_COUNTS } from "./labels";

/**
 * The engine's comparison of audit anchors with the audit trail (design brief §4), as counts.
 * `null` means no anchors were compared.
 */
export function AnchorCounts({ summary }: { summary: AnchorSummary | null }) {
  if (summary === null) return <p>No audit anchors were compared.</p>;
  return (
    <Facts
      items={ANCHOR_COUNTS.map(({ key, label }) => ({ label, value: String(summary[key]) }))}
    />
  );
}
