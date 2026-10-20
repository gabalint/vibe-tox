import { useParams } from "react-router";
import type { ApproveOrderableVersion } from "@/generated/ApproveOrderableVersion";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import type { OrderableView } from "@/generated/OrderableView";
import type { RetireOrderableVersion } from "@/generated/RetireOrderableVersion";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { orderablePath } from "../catalogColumns";
import {
  GET_ORDERABLE_VERSION,
  LIST_ORDERABLE_VERSIONS,
  LIST_ORDERABLES,
  ORDERABLE_QUERIES,
} from "../contract";
import { ORDERABLE_KINDS } from "../labels";
import type { VersionWorkflow } from "../VersionActions";
import { VersionedRecordPage } from "../VersionedRecordPage";
import type { VersionDetailQuery } from "../VersionPanel";
import { OrderableContent } from "./OrderableContent";
import { OrderableDraftDialog } from "./OrderableDraftDialog";

const ORDERABLE_WORKFLOW: VersionWorkflow<ApproveOrderableVersion, RetireOrderableVersion> = {
  noun: "test version",
  approveCommand: "approve_orderable_version",
  retireCommand: "retire_orderable_version",
  invalidates: ORDERABLE_QUERIES,
  approveInput: (version, selfApproval) => ({
    orderableVersionId: version.id,
    expectedRevision: version.revision,
    selfApproval,
  }),
  retireInput: (version, reason) => ({
    orderableVersionId: version.id,
    expectedRevision: version.revision,
    reason,
  }),
};

const ORDERABLES_HOME = { label: "Tests and panels", href: "/configuration/orderables" };

const DETAIL: VersionDetailQuery = {
  command: GET_ORDERABLE_VERSION,
  args: (orderableVersionId: string) => ({ input: { orderableVersionId } }),
};

/**
 * One test or panel and its versions (REQ-ANA-009). Order entry offers only the version in
 * effect; orders keep the version they were placed with.
 */
export function OrderablePage() {
  const { orderableId = "" } = useParams();
  const orderables = useCommandQuery<OrderableView[]>(LIST_ORDERABLES);
  const versions = useCommandQuery<CatalogVersionSummary[]>(LIST_ORDERABLE_VERSIONS, {
    input: { orderableId },
  });
  const orderable = orderables.data?.find((each) => each.id === orderableId);
  return (
    <VersionedRecordPage<ApproveOrderableVersion, RetireOrderableVersion, OrderableVersionView>
      recordId={orderableId}
      records={orderables}
      home={ORDERABLES_HOME}
      recordPath={orderablePath}
      genericTitle="Test or panel"
      description={orderable && labelOf(ORDERABLE_KINDS, orderable.kind)}
      versions={versions}
      workflow={ORDERABLE_WORKFLOW}
      detail={DETAIL}
      renderContent={(view) => <OrderableContent view={view} />}
      renderDraftDialog={(mode, onSaved, onClose) => (
        <OrderableDraftDialog
          orderableId={orderableId}
          mode={mode}
          onSaved={onSaved}
          onClose={onClose}
        />
      )}
    />
  );
}
