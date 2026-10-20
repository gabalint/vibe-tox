import { Alert } from "@/components/Alert";
import { GateReasons } from "@/components/ErrorState";
import { StatusBadge } from "@/components/StatusBadge";
import type { GateReason } from "@/generated/GateReason";
import styles from "@/styles/layout.module.css";
import type { DispositionView } from "../contract";
import { DISPOSITION } from "../labels";

/**
 * Each expected test and the engine's disposition for it (REQ-RPT-005): reported, pending,
 * held, cancelled, not activated or awaiting a reflex decision, with what holds it.
 */
export function Dispositions({ dispositions }: { dispositions: readonly DispositionView[] }) {
  if (dispositions.length === 0) return <p className={styles.hint}>No expected tests.</p>;
  return (
    <ul aria-label="Expected tests" className={styles.stack}>
      {dispositions.map((item) => {
        const status = DISPOSITION[item.disposition];
        return (
          <li key={item.componentId ?? item.name}>
            {item.name} <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
            {item.gates.length > 0 && (
              <GateReasons label={`Holding ${item.name}`} gates={item.gates} />
            )}
          </li>
        );
      })}
    </ul>
  );
}

/**
 * What blocks release, in the engine's words, as a message at the top of the screen; nothing
 * when the engine returned no gate. The screen adds no rule of its own (UI review finding 4).
 */
export function ReleaseBlocked({ gates }: { gates: readonly GateReason[] }) {
  if (gates.length === 0) return null;
  return (
    <Alert tone="hold" title="Release is blocked until these are resolved">
      <GateReasons label="Blocking release" gates={gates} />
    </Alert>
  );
}
