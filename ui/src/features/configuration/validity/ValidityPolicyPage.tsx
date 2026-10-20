import { useParams } from "react-router";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import type { ApproveValidityPolicyVersion } from "@/generated/ApproveValidityPolicyVersion";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { RetireValidityPolicyVersion } from "@/generated/RetireValidityPolicyVersion";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";
import type { ValidityCategory } from "@/generated/ValidityCategory";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { specimenTypePath } from "../catalogColumns";
import {
  GET_VALIDITY_POLICY,
  LIST_SPECIMEN_TYPES,
  LIST_VALIDITY_POLICY_VERSIONS,
  VALIDITY_POLICY_QUERIES,
} from "../contract";
import { VALIDITY_ACTIONS } from "../labels";
import type { VersionWorkflow } from "../VersionActions";
import { VersionedRecordPage } from "../VersionedRecordPage";
import type { VersionDetailQuery } from "../VersionPanel";
import { ValidityDraftDialog } from "./ValidityDraftDialog";

const VALIDITY_WORKFLOW: VersionWorkflow<
  ApproveValidityPolicyVersion,
  RetireValidityPolicyVersion
> = {
  noun: "validity policy version",
  approveCommand: "approve_validity_policy_version",
  retireCommand: "retire_validity_policy_version",
  invalidates: VALIDITY_POLICY_QUERIES,
  approveInput: (version, selfApproval) => ({
    policyVersionId: version.id,
    expectedRevision: version.revision,
    selfApproval,
  }),
  retireInput: (version, reason) => ({
    policyVersionId: version.id,
    expectedRevision: version.revision,
    reason,
  }),
};

const SPECIMEN_TYPES_HOME = { label: "Specimen types", href: "/configuration/specimen-types" };

const DETAIL: VersionDetailQuery = {
  command: GET_VALIDITY_POLICY,
  args: (policyVersionId: string) => ({ input: { policyVersionId } }),
};

/**
 * One specimen type and its validity policy versions (REQ-SPC-008): the validity categories,
 * each with the action the laboratory approved.
 */
export function ValidityPolicyPage() {
  const { specimenTypeId = "" } = useParams();
  const specimenTypes = useCommandQuery<SpecimenTypeView[]>(LIST_SPECIMEN_TYPES);
  const versions = useCommandQuery<CatalogVersionSummary[]>(LIST_VALIDITY_POLICY_VERSIONS, {
    input: { specimenTypeId },
  });
  return (
    <VersionedRecordPage<
      ApproveValidityPolicyVersion,
      RetireValidityPolicyVersion,
      ValidityPolicyView
    >
      recordId={specimenTypeId}
      records={specimenTypes}
      home={SPECIMEN_TYPES_HOME}
      recordPath={specimenTypePath}
      genericTitle="Specimen type"
      description="Validity policy versions: the validity categories of this specimen type, each with the action the laboratory approved."
      versions={versions}
      workflow={VALIDITY_WORKFLOW}
      detail={DETAIL}
      renderContent={(view) => (
        <DataGrid
          label={`Categories of version ${view.version}`}
          columns={CATEGORY_COLUMNS}
          rows={view.categories}
          getRowId={(category) => category.code}
          fill
        />
      )}
      renderDraftDialog={(mode, onSaved, onClose) => (
        <ValidityDraftDialog
          specimenTypeId={specimenTypeId}
          mode={mode}
          onSaved={onSaved}
          onClose={onClose}
        />
      )}
    />
  );
}

const CATEGORY_COLUMNS: readonly DataGridColumn<ValidityCategory>[] = [
  { id: "code", header: "Code", cell: (row) => row.code, sortValue: (row) => row.code, width: 180 },
  { id: "label", header: "Label", cell: (row) => row.label },
  {
    id: "action",
    header: "Action",
    cell: (row) => labelOf(VALIDITY_ACTIONS, row.action),
    width: 220,
  },
  { id: "qualifier", header: "Qualifier text", cell: (row) => row.qualifierText ?? "None" },
];
