import styles from "./IntakeSteps.module.css";

const STEPS = [
  { id: "patient", label: "Patient" },
  { id: "order", label: "Order details" },
  { id: "receive", label: "Receive specimens" },
  { id: "finish", label: "Finish receipt" },
] as const;

/** Task position, not a clinical status or an assertion that any safety gate has passed. */
export function IntakeSteps({ current }: { current: (typeof STEPS)[number]["id"] }) {
  return (
    <ol className={styles.steps} aria-label="Intake progress">
      {STEPS.map((step, index) => (
        <li
          key={step.id}
          className={styles.step}
          aria-current={step.id === current ? "step" : undefined}
        >
          <span className={styles.number} aria-hidden="true">
            {index + 1}
          </span>
          {step.label}
        </li>
      ))}
    </ol>
  );
}
