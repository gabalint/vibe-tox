import type { GateReason } from "@/generated/GateReason";
import { gateLabel } from "@/lib/errors";
import styles from "./ErrorState.module.css";

/** Gates that block an action: each code's plain label, then the engine's own message. */
export function GateReasons({ label, gates }: { label: string; gates: readonly GateReason[] }) {
  return (
    <ul aria-label={label} className={styles.gates}>
      {gates.map((gate) => {
        const name = gateLabel(gate.code);
        return (
          <li key={`${gate.code}:${gate.message}`}>
            {name && <span className={styles.gateLabel}>{name}:</span>} {gate.message}
          </li>
        );
      })}
    </ul>
  );
}
