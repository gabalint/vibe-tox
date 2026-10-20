import { useNavigate } from "react-router";
import type { DerivationView } from "@/generated/DerivationView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { DERIVATION_QUERIES } from "./contract";
import { derivationPath } from "./paths";

interface DeriveInput {
  subjectId: string;
  dateOfService: string | null;
}

interface DeriveDialogProps {
  input: DeriveInput;
  onClose: () => void;
}

/**
 * Derives the billable services for one patient and date of service from the work the
 * laboratory performed and reported (REQ-BIL-005), then opens the derivation. Deriving again
 * with nothing changed returns the same derivation.
 */
export function DeriveDialog({ input, onClose }: DeriveDialogProps) {
  const navigate = useNavigate();
  return (
    <CommandConfirmDialog<DeriveInput, DerivationView>
      title="Derive billable services?"
      confirmLabel="Derive services"
      variant="primary"
      command={{ name: "derive_services", invalidates: DERIVATION_QUERIES }}
      input={input}
      successTitle="Services derived"
      onDone={(derivation) => navigate(derivationPath(derivation.derivationId))}
      onClose={onClose}
    >
      <p>
        The services are derived from the work performed and reported for this patient on the date
        of service, never from positive results. A new derivation replaces an earlier one not yet
        handed off, and adjusts the latest derivation that was approved.
      </p>
    </CommandConfirmDialog>
  );
}
