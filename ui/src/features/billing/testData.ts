import type { AnalyteView } from "@/generated/AnalyteView";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import type { BillingClassMappingVersionView } from "@/generated/BillingClassMappingVersionView";
import type { BillingWorkItem } from "@/generated/BillingWorkItem";
import type { DerivationView } from "@/generated/DerivationView";
import type { HandoffView } from "@/generated/HandoffView";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SubjectSummary } from "@/generated/SubjectSummary";

/** Synthetic billing records for the billing tests. No real laboratory or patient data. */

const ME = "00000000-0000-7000-8000-0000000000e1";
export const AUTHOR = "00000000-0000-7000-8000-0000000000e2";

export function sessionWith(capabilities: string[]): SessionInfo {
  return {
    userId: ME,
    username: "billing",
    displayName: "Test Billing User",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

export const AMPHETAMINE: AnalyteView = {
  id: "00000000-0000-7000-8000-0000000000a1",
  code: "SYN-AMP",
  name: "Synthetic amphetamine",
  loincCode: null,
  drugClassCode: null,
};

export const OPIATE: AnalyteView = {
  id: "00000000-0000-7000-8000-0000000000a2",
  code: "SYN-OPI",
  name: "Synthetic opiate",
  loincCode: null,
  drugClassCode: null,
};

export const CURRENT: BillingClassMappingVersionSummary = {
  versionId: "00000000-0000-7000-8000-0000000000b1",
  version: 1,
  status: "approved",
  revision: 3,
  createdBy: AUTHOR,
  createdAt: "2026-10-01T09:00:00Z",
  approvedBy: ME,
  approvedAt: "2026-10-02T09:00:00Z",
  confirmedBy: ME,
  confirmedAt: "2026-10-03T09:00:00Z",
  effectiveFrom: "2026-10-03T09:00:00Z",
  effectiveTo: null,
  retiredBy: null,
  retiredAt: null,
  retireReason: null,
};

export const DRAFT: BillingClassMappingVersionSummary = {
  ...CURRENT,
  versionId: "00000000-0000-7000-8000-0000000000b2",
  version: 2,
  status: "draft",
  revision: 1,
  createdBy: ME,
  createdAt: "2026-10-12T09:00:00Z",
  approvedBy: null,
  approvedAt: null,
  confirmedBy: null,
  confirmedAt: null,
  effectiveFrom: null,
};

export const CURRENT_VIEW: BillingClassMappingVersionView = {
  ...CURRENT,
  entries: [{ analyteId: AMPHETAMINE.id, classCode: "SYN-C1", classLabel: "Synthetic class 1" }],
  allowedActions: ["retire_class_mapping_version"],
};

export const DRAFT_VIEW: BillingClassMappingVersionView = {
  ...DRAFT,
  entries: [
    { analyteId: AMPHETAMINE.id, classCode: "SYN-C1", classLabel: "Synthetic class 1" },
    {
      analyteId: "00000000-0000-7000-8000-0000000000a9",
      classCode: "SYN-C9",
      classLabel: "Synthetic class 9",
    },
  ],
  allowedActions: ["update_class_mapping_draft", "approve_class_mapping_version"],
};

export const SUBJECT: SubjectSummary = {
  subjectId: "00000000-0000-7000-8000-0000000000c1",
  revision: 1,
  familyName: "Patient",
  givenNames: "Synthetic",
  birthDate: "1980-01-01",
  administrativeSex: null,
  identifiers: [
    {
      id: "00000000-0000-7000-8000-0000000000c2",
      assigningAuthority: "SYN-LAB",
      identifierType: "MR",
      value: "MRN-00000",
      active: true,
      addedAt: "2026-10-01T08:00:00Z",
      deactivatedAt: null,
      deactivationReason: null,
    },
  ],
  openIdentityHolds: 0,
};

export const DERIVATION_ID = "00000000-0000-7000-8000-0000000000d1";
export const HANDOFF_ID = "00000000-0000-7000-8000-0000000000f1";

export function workEntry(overrides: Partial<BillingWorkItem> = {}): BillingWorkItem {
  return {
    subjectId: SUBJECT.subjectId,
    dateOfService: "2026-10-10",
    accessionNumbers: ["A26000001"],
    derivationId: null,
    handoffId: null,
    handoffNo: null,
    itemId: null,
    state: "needs_derivation",
    since: "2026-10-11T15:00:00Z",
    allowedActions: ["derive_services"],
    ...overrides,
  };
}

export const DERIVATION: DerivationView = {
  derivationId: DERIVATION_ID,
  derivationNo: 2,
  revision: 4,
  state: "held",
  completeness: "complete",
  subject: SUBJECT,
  dateOfService: "2026-10-10",
  dosRule: "collection_local_date_v1",
  mappingVersion: { versionId: CURRENT.versionId, version: 1 },
  specimens: [
    {
      specimenId: "00000000-0000-7000-8000-0000000000e8",
      accessionNumber: "A26000001",
      orderId: "00000000-0000-7000-8000-0000000000e9",
      orderVersion: 3,
      reportRevisionId: "00000000-0000-7000-8000-0000000000e7",
      classes: [
        {
          classCode: "SYN-C1",
          classLabel: "Synthetic class 1",
          certainty: "presumptive",
          ordered: true,
          performed: true,
          reported: false,
        },
      ],
      components: [
        {
          componentId: "00000000-0000-7000-8000-0000000000ea",
          orderable: "Synthetic screen panel",
          disposition: "failed_only",
        },
      ],
    },
  ],
  flags: [
    { code: "mapping_missing", refId: OPIATE.id, blocking: true },
    { code: "performed_not_reported", refId: "", blocking: false },
  ],
  flagReferenceLabels: { [OPIATE.id]: "Synthetic opiate analyte" },
  adjustmentOf: { derivationId: "00000000-0000-7000-8000-0000000000d0", derivationNo: 1 },
  stale: false,
  allowedActions: ["derive_services"],
};

export const HANDOFF: HandoffView = {
  handoffId: HANDOFF_ID,
  handoffNo: 7,
  format: "bil-handoff-v1",
  artifactSha256: "0000000000000000000000000000000000000000000000000000000000000001",
  itemCount: 2,
  createdBy: ME,
  createdAt: "2026-10-12T16:00:00Z",
  items: [
    {
      itemId: "00000000-0000-7000-8000-0000000000f2",
      derivationId: DERIVATION_ID,
      derivationNo: 2,
      kind: "original",
      adjustsItemId: null,
      status: "outstanding",
      outcome: null,
      allowedActions: ["record_handoff_outcome"],
    },
    {
      itemId: "00000000-0000-7000-8000-0000000000f3",
      derivationId: "00000000-0000-7000-8000-0000000000d2",
      derivationNo: 3,
      kind: "adjustment",
      adjustsItemId: "00000000-0000-7000-8000-0000000000f0",
      status: "rejected",
      outcome: {
        outcome: "rejected",
        externalReference: null,
        reason: "Synthetic partner rejection",
        recordedBy: ME,
        recordedAt: "2026-10-12T17:00:00Z",
      },
      allowedActions: [],
    },
  ],
  allowedActions: ["export_billing_handoff"],
};
