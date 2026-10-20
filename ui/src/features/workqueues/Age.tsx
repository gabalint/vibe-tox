import { Timestamp } from "@/components/Timestamp";
import { ageText } from "./ageText";

/** How long an item has waited, as of this render, with the time it started waiting. */
export function Age({ since }: { since: string | null }) {
  if (since === null) return "—";
  return (
    <span>
      {ageText(since, Date.now())} (since <Timestamp value={since} />)
    </span>
  );
}
