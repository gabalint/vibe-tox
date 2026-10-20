import { useState } from "react";
import { Button } from "@/components/Button";
import { LEAVE_DEMO_LABORATORY, RESET_DEMO_LABORATORY } from "./contract";
import styles from "./DemoBanner.module.css";
import { SwitchDialog } from "./SwitchDialog";

type Open = "leave" | "reset" | null;

/**
 * The permanent mark of the demo laboratory on every screen, with the ways out of it
 * (proposed REQ-OPS-016; starter data brief, decision 1a). Reports and labels printed here
 * carry their own DEMO marking.
 */
export function DemoBanner() {
  const [open, setOpen] = useState<Open>(null);
  const close = () => setOpen(null);
  return (
    <section aria-label="Demo laboratory" className={styles.banner}>
      <p className={styles.text}>
        <strong>DEMO LABORATORY — not for patient testing</strong>
        <span className={styles.detail}>
          Fictitious data in its own data folder, separate from the laboratory's.
        </span>
      </p>
      <div className={styles.actions}>
        <Button onPress={() => setOpen("leave")}>Leave the demo laboratory</Button>
        <Button onPress={() => setOpen("reset")}>Reset the demo laboratory</Button>
      </div>
      {open === "leave" && (
        <SwitchDialog
          title="Leave the demo laboratory?"
          confirmLabel="Leave the demo laboratory"
          variant="primary"
          command={LEAVE_DEMO_LABORATORY}
          onClose={close}
        >
          <p>
            The app restarts and opens the laboratory's own installation. The demo laboratory is
            kept until it is reset.
          </p>
        </SwitchDialog>
      )}
      {open === "reset" && (
        <SwitchDialog
          title="Reset the demo laboratory?"
          confirmLabel="Delete the demo data"
          variant="danger"
          command={RESET_DEMO_LABORATORY}
          onClose={close}
        >
          <p>
            Everything in the demo laboratory is deleted: its users, demo data and anything entered
            here. The app then restarts with an empty demo laboratory. The laboratory's own data is
            not touched.
          </p>
        </SwitchDialog>
      )}
    </section>
  );
}
