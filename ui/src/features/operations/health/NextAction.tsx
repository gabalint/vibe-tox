import { Link } from "react-router";
import type { HealthCheckView } from "@/generated/HealthCheckView";
import { useHasCapability } from "@/lib/commands";
import { checkFix } from "./checkFixes";
import { NONE } from "./healthLabels";

/**
 * The engine's next action, with a link to the screen where it is done when that is in the app
 * and the person may act there. An action done outside the app stays text.
 */
export function NextAction({ check }: { check: HealthCheckView }) {
  const fix = checkFix(check.check);
  const canAct = useHasCapability(fix?.capability ?? "");
  if (check.nextAction === null) return NONE;
  return (
    <>
      <div>{check.nextAction}</div>
      {fix && canAct && (
        <div>
          <Link to={fix.path}>{`Open ${fix.place}`}</Link>
        </div>
      )}
    </>
  );
}
