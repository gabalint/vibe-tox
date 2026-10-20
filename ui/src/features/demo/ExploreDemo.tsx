import { useState } from "react";
import { Button } from "@/components/Button";
import styles from "@/styles/layout.module.css";
import { OPEN_DEMO_LABORATORY, useInstallation } from "./contract";
import { SwitchDialog } from "./SwitchDialog";

/**
 * On the setup screen of a laboratory installation that is not set up yet: the way into the
 * demo laboratory (proposed REQ-OPS-016). Once the laboratory is set up the setup screen is
 * gone, and the shell refuses the switch, so a working laboratory is never left for the demo.
 */
export function ExploreDemo() {
  const [open, setOpen] = useState(false);
  if (useInstallation() !== "laboratory") return null;
  return (
    <div className={styles.stack}>
      <div>
        <Button variant="primary" onPress={() => setOpen(true)}>
          Explore the demo laboratory
        </Button>
      </div>
      <p>The app restarts in the demo before you enter your account details.</p>
      {open && (
        <SwitchDialog
          title="Open the demo laboratory?"
          confirmLabel="Open the demo laboratory"
          variant="primary"
          command={OPEN_DEMO_LABORATORY}
          onClose={() => setOpen(false)}
        >
          <p>
            The app restarts in the separate demo laboratory. Create your account there and choose
            to load the Standard Catalog and demo data automatically after sign-in. Any details
            entered on this screen will not be saved. You can leave or reset the demo at any time.
          </p>
        </SwitchDialog>
      )}
    </div>
  );
}
