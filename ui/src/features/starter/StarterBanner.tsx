import { useState } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import type { StandardCatalogStatus } from "@/generated/StandardCatalogStatus";
import { CATALOG_REVIEW_PATH } from "@/lib/catalogQueries";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { APPROVE_CONFIGURATION, MANAGE_CONFIGURATION, STANDARD_CATALOG_STATUS } from "./contract";

/**
 * The notice every signed-in person sees while loaded Standard Catalog drafts await review
 * (REQ-OPS-015; starter data brief, disclaimer). It clears when catalog review or single
 * approvals leave no loaded draft. People who can act get a link to Catalog review.
 * Dismissal lasts until the signed-in shell unmounts; it does not approve any drafts.
 * A failed read shows no notice.
 */
export function StarterBanner() {
  const [dismissed, setDismissed] = useState(false);
  const status = useCommandQuery<StandardCatalogStatus>(STANDARD_CATALOG_STATUS);
  const canManage = useHasCapability(MANAGE_CONFIGURATION);
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  if (dismissed || !status.data || status.data.draftsRemaining === 0) return null;
  return (
    <Alert
      tone="warning"
      title="Starter catalog content is not yet reviewed: not ready for patient testing"
      onDismiss={() => setDismissed(true)}
    >
      {(canManage || canApprove) && <Link to={CATALOG_REVIEW_PATH}>Catalog review</Link>}
    </Alert>
  );
}
