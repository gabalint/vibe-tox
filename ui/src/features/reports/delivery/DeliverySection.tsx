import { useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { Section } from "@/components/Section";
import styles from "@/styles/layout.module.css";
import type { ObligationView } from "../contract";
import { obligationPath } from "../paths";
import { ObligationDialog } from "./ObligationDialog";
import { ObligationGrid } from "./ObligationGrid";
import { useObligationActions } from "./obligationActions";

interface DeliverySectionProps {
  obligations: readonly ObligationView[];
  /** The order's client, whose destinations may be added. */
  clientId: string;
  /** The current revision, when the signed-in person may add a destination for it. */
  currentRevisionId: string | undefined;
}

/**
 * A report's delivery obligations, with each one's status and last attempt, and the actions
 * the engine allows on it in its row menu (REQ-RPT-003, REQ-RPT-011). A row opens its
 * obligation in the delivery workspace, which holds every obligation not yet closed. A
 * closed one has no view beyond its row: the engine has no read of one obligation by ID.
 */
export function DeliverySection({
  obligations,
  clientId,
  currentRevisionId,
}: DeliverySectionProps) {
  const [isAdding, setAdding] = useState(false);
  const actions = useObligationActions();
  const navigate = useNavigate();
  const open = (row: ObligationView) => {
    if (row.status.code !== "closed") void navigate(obligationPath(row.obligationId));
  };
  return (
    <Section
      title="Delivery"
      actions={
        currentRevisionId && <Button onPress={() => setAdding(true)}>Add destination</Button>
      }
    >
      {obligations.length === 0 ? (
        <p className={styles.hint}>No delivery obligations.</p>
      ) : (
        <ObligationGrid obligations={obligations} rowActions={actions.rowActions} onOpen={open} />
      )}
      {actions.dialog}
      {isAdding && currentRevisionId && (
        <ObligationDialog
          mode={{ kind: "add", revisionId: currentRevisionId, clientId }}
          onClose={() => setAdding(false)}
        />
      )}
    </Section>
  );
}
