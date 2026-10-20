import { Link } from "react-router";
import { Button } from "@/components/Button";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { ConfirmDistinctSubjects } from "@/generated/ConfirmDistinctSubjects";
import type { IdentityHoldResolved } from "@/generated/IdentityHoldResolved";
import type { IdentityHoldView } from "@/generated/IdentityHoldView";
import { useCommandMutation } from "@/lib/commands";
import styles from "../accessioning.module.css";
import { GET_SUBJECT } from "../contract";
import { HoldsSection } from "../HoldsSection";
import { patientPath } from "../subjects";
import { RESOLVE_IDENTITY_HOLD, SEARCH_SUBJECTS } from "./contract";

interface IdentityHoldsProps {
  subjectId: string;
  /** Open holds on this patient or listing it as a possible match. */
  holds: readonly IdentityHoldView[];
  /** The session holds `manage_quality`; the engine checks it again. */
  canResolve: boolean;
}

/**
 * Open identity holds (REQ-IDN-001). While one is open, the results of every patient it
 * names cannot be released.
 */
export function IdentityHolds({ subjectId, holds, canResolve }: IdentityHoldsProps) {
  return (
    <HoldsSection
      title="Identity holds"
      emptyText="No open identity holds."
      holds={holds}
      holdId={(hold) => hold.id}
      renderHold={(hold, resolve) => (
        <>
          <div className={styles.listRow}>
            <StatusBadge tone="hold">Possible duplicate</StatusBadge>
            <span className={styles.hint}>
              Opened <Timestamp value={hold.openedAt} />
            </span>
          </div>
          <p>
            Compare with:{" "}
            {otherRecords(hold, subjectId).map((id, index) => (
              <span key={id}>
                {index > 0 && ", "}
                <Link to={patientPath(id)}>patient record {index + 1}</Link>
              </span>
            ))}
          </p>
          {canResolve && (
            <div>
              <Button onPress={resolve}>Confirm different people</Button>
            </div>
          )}
        </>
      )}
      renderResolution={(hold, close) => <ResolveHold hold={hold} onClose={close} />}
    />
  );
}

function ResolveHold({ hold, onClose }: { hold: IdentityHoldView; onClose: () => void }) {
  const resolve = useCommandMutation<IdentityHoldResolved, { input: ConfirmDistinctSubjects }>(
    RESOLVE_IDENTITY_HOLD,
    { invalidates: [GET_SUBJECT, SEARCH_SUBJECTS] },
  );
  return (
    <ReasonConfirmDialog
      title="Confirm these are different people?"
      confirmLabel="Confirm different people"
      variant="primary"
      command={resolve}
      input={(reason) => ({ holdId: hold.id, expectedRevision: hold.revision, reason })}
      successTitle="Identity hold resolved"
      onClose={onClose}
    >
      <p>
        The hold is resolved and the records stay separate. State the evidence that they belong to
        different people, such as a different identifier or birth date on the source document.
      </p>
    </ReasonConfirmDialog>
  );
}

/** The other patients a hold names: its subject and candidates, without the one shown. */
function otherRecords(hold: IdentityHoldView, subjectId: string): string[] {
  return [hold.subjectId, ...hold.candidateSubjectIds].filter((id) => id !== subjectId);
}
