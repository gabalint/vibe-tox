import { StatusBadge } from "@/components/StatusBadge";
import type { ExecutionView } from "@/generated/ExecutionView";

/** A patient attempt's state, with the reason it failed as entered. */
export function ExecutionState({ execution }: { execution: ExecutionView | null }) {
  if (execution === null) return "—";
  if (execution.state === "active") return <StatusBadge tone="neutral">Active</StatusBadge>;
  return (
    <>
      <StatusBadge tone="danger">Failed</StatusBadge> {execution.stateReason ?? ""}
    </>
  );
}
