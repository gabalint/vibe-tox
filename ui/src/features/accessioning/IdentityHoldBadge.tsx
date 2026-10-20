import { StatusBadge } from "@/components/StatusBadge";

/** Open identity holds block release of the patient's results (REQ-IDN-001). */
export function IdentityHoldBadge({ count }: { count: number }) {
  if (count === 0) return <StatusBadge tone="neutral">No identity hold</StatusBadge>;
  return (
    <StatusBadge tone="hold">
      {count === 1 ? "Identity hold open" : `${count} identity holds open`}
    </StatusBadge>
  );
}
