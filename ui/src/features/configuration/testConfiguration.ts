import type { AnalyteView } from "@/generated/AnalyteView";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import type { MethodView } from "@/generated/MethodView";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SpecimenTypeView } from "@/generated/SpecimenTypeView";

/** Synthetic catalog records for the configuration tests. No real laboratory settings. */
const AUTHOR_ID = "00000000-0000-7000-8000-000000000901";
export const APPROVER_ID = "00000000-0000-7000-8000-000000000902";

export function sessionWith(capabilities: string[], userId = AUTHOR_ID): SessionInfo {
  return {
    userId,
    username: "config",
    displayName: "Test Configurer",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

export const MANAGER = ["manage_configuration"];
export const APPROVER = ["approve_configuration"];

export const URINE: SpecimenTypeView = {
  id: "00000000-0000-7000-8000-000000000a01",
  code: "urine",
  name: "Urine",
  versionInEffect: null,
  draftVersion: null,
};

export const ANALYTES: AnalyteView[] = [
  {
    id: "00000000-0000-7000-8000-000000000b01",
    code: "SYN-AMP",
    name: "Synthetic amphetamine",
    loincCode: null,
    drugClassCode: "SYN-STIM",
  },
  {
    id: "00000000-0000-7000-8000-000000000b02",
    code: "SYN-CRE",
    name: "Synthetic creatinine",
    loincCode: null,
    drugClassCode: null,
  },
];

export const METHOD: MethodView = {
  id: "00000000-0000-7000-8000-000000000c01",
  code: "SYN-IA",
  name: "Synthetic immunoassay",
  versionInEffect: 1,
  draftVersion: null,
};

const VERSION_BASE = {
  revision: 1,
  createdBy: AUTHOR_ID,
  createdAt: "2026-10-12T09:00:00Z",
  approvedBy: null,
  effectiveFrom: null,
  effectiveTo: null,
  replacedBy: null,
  retiredBy: null,
  retiredAt: null,
  retireReason: null,
};

export const APPROVED_VERSION: CatalogVersionSummary = {
  ...VERSION_BASE,
  id: "00000000-0000-7000-8000-000000000d01",
  version: 1,
  status: "approved",
  approvedBy: APPROVER_ID,
  approvedByDisplay: "Test Approver",
  effectiveFrom: "2026-10-12T10:00:00Z",
};

export const DRAFT_VERSION: CatalogVersionSummary = {
  ...VERSION_BASE,
  id: "00000000-0000-7000-8000-000000000d02",
  version: 2,
  status: "draft",
  createdAt: "2026-10-12T11:00:00Z",
};

/** A method version; the limits are exact decimal text with trailing zeros kept. */
export function methodVersion(summary: CatalogVersionSummary): MethodVersionView {
  return {
    ...summary,
    methodId: METHOD.id,
    reportNotes: null,
    methodCode: METHOD.code,
    methodName: METHOD.name,
    specimenTypeId: URINE.id,
    specimenTypeCode: URINE.code,
    certainty: "presumptive",
    outputScale: "qualitative",
    thresholdRule: "at_or_above",
    thresholdRounding: "before_rounding",
    analytes: [
      {
        analyteId: ANALYTES[0]?.id ?? "",
        analyteCode: "SYN-AMP",
        analyteName: "Synthetic amphetamine",
        unit: "ng/mL",
        cutoff: "500.0",
        limitOfDetection: "25.00",
        limitOfQuantitation: "50",
        reportableLow: "50",
        reportableHigh: "5000",
        rangePolicy: "report_only",
        referenceInterval: null,
        displayScale: 1,
      },
    ],
  };
}

export const VERSION_REF = { id: "00000000-0000-7000-8000-000000000d03", version: 3, revision: 1 };
