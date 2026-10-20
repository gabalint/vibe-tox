import { useParams } from "react-router";
import type { ApproveMethodVersion } from "@/generated/ApproveMethodVersion";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import type { MethodView } from "@/generated/MethodView";
import type { RetireMethodVersion } from "@/generated/RetireMethodVersion";
import { useCommandQuery } from "@/lib/commands";
import { methodPath } from "../catalogColumns";
import {
  GET_METHOD_VERSION,
  LIST_METHOD_VERSIONS,
  LIST_METHODS,
  METHOD_QUERIES,
} from "../contract";
import type { VersionWorkflow } from "../VersionActions";
import { VersionedRecordPage } from "../VersionedRecordPage";
import type { VersionDetailQuery } from "../VersionPanel";
import { MethodContent } from "./MethodContent";
import { MethodDraftDialog } from "./MethodDraftDialog";

const METHOD_WORKFLOW: VersionWorkflow<ApproveMethodVersion, RetireMethodVersion> = {
  noun: "method version",
  approveCommand: "approve_method_version",
  retireCommand: "retire_method_version",
  invalidates: METHOD_QUERIES,
  externalApprovalReference: true,
  approveInput: (version, selfApproval, reference) => ({
    externalApprovalReference: reference,
    methodVersionId: version.id,
    expectedRevision: version.revision,
    selfApproval,
  }),
  retireInput: (version, reason) => ({
    methodVersionId: version.id,
    expectedRevision: version.revision,
    reason,
  }),
};

const METHODS_HOME = { label: "Methods", href: "/configuration/methods" };

const DETAIL: VersionDetailQuery = {
  command: GET_METHOD_VERSION,
  args: (methodVersionId: string) => ({ input: { methodVersionId } }),
};

/**
 * One method and its versions (REQ-ANA-002, REQ-ANA-009). The engine refuses to retire or
 * replace a version that current tests still use.
 */
export function MethodPage() {
  const { methodId = "" } = useParams();
  const methods = useCommandQuery<MethodView[]>(LIST_METHODS);
  const versions = useCommandQuery<CatalogVersionSummary[]>(LIST_METHOD_VERSIONS, {
    input: { methodId },
  });
  return (
    <VersionedRecordPage<ApproveMethodVersion, RetireMethodVersion, MethodVersionView>
      recordId={methodId}
      records={methods}
      home={METHODS_HOME}
      recordPath={methodPath}
      genericTitle="Method"
      versions={versions}
      workflow={METHOD_WORKFLOW}
      detail={DETAIL}
      renderContent={(view) => <MethodContent view={view} />}
      renderDraftDialog={(mode, onSaved, onClose) => (
        <MethodDraftDialog methodId={methodId} mode={mode} onSaved={onSaved} onClose={onClose} />
      )}
    />
  );
}
