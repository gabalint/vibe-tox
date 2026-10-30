import type { GateReason } from "@/generated/GateReason";
import { gateLabel } from "@/lib/errors";
import styles from "./ErrorState.module.css";

/**
 * Gates that block an action: each code's plain label, then the engine's own message.
 * The engine returns one gate per causal record, so identical gates show once.
 */
export function GateReasons({ label, gates }: { label: string; gates: readonly GateReason[] }) {
  const unique = new Map(gates.map((gate) => [`${gate.code}:${gate.message}`, gate]));
  return (
    <ul aria-label={label} className={styles.gates}>
      {[...unique].map(([key, gate]) => {
        const name = gateLabel(gate.code);
        return (
          <li key={key}>
            {name && <span className={styles.gateLabel}>{name}:</span>} {gate.message}
          </li>
        );
      })}
    </ul>
  );
}
