import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LotView } from "@/generated/LotView";
import type { MeasurementView } from "@/generated/MeasurementView";
import type { ResultRevisionView } from "@/generated/ResultRevisionView";
import type { RunDetail } from "@/generated/RunDetail";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import type { SubjectSummary } from "@/generated/SubjectSummary";

/** Synthetic bench and review records for the analytics tests. No real laboratory data. */

export function sessionWith(capabilities: string[]): SessionInfo {
  return {
    userId: "00000000-0000-7000-8000-000000000e01",
    username: "bench",
    displayName: "Test Analyst",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

export const INSTRUMENT: InstrumentReadinessView = {
  instrumentId: "00000000-0000-7000-8000-000000001001",
  name: "Synthetic analyzer 1",
  status: "in_service",
  since: "2026-10-12T08:00:00Z",
  transport: "astm_tcp",
  activeRegistration: {
    registrationId: "00000000-0000-7000-8000-000000002001",
    sender: "SYN-SENDER",
    version: "1.0",
    fileTemplate: false,
  },
};

export const IDLE_INSTRUMENT: InstrumentReadinessView = {
  instrumentId: "00000000-0000-7000-8000-000000001002",
  name: "Synthetic analyzer 2",
  status: null,
  since: null,
  transport: "astm_serial",
  activeRegistration: null,
};

export const ANALYTE = {
  id: "00000000-0000-7000-8000-000000000b01",
  code: "SYN-AMP",
  name: "Synthetic amphetamine",
  loincCode: null,
  drugClassCode: null,
};

export const DRAFT_SOURCE: SourceRegistrationView = {
  registrationId: "00000000-0000-7000-8000-000000002002",
  instrumentId: INSTRUMENT.instrumentId,
  senderName: "SYN-SENDER",
  senderVersion: "2.0",
  sampleIdField: "specimen_id",
  revisionIndicator: "result_status_correction",
  reportsFinalConcentration: false,
  state: "draft",
  authoredBy: "author-1",
  authoredAt: "2026-10-12T09:00:00Z",
  approvedBy: null,
  approvedAt: null,
  retiredAt: null,
  revision: 1,
  testCodes: [{ sourceTestCode: "AMP", analyteId: ANALYTE.id }],
  fileLayout: null,
  allowedActions: ["approve_source"],
};

export const CONTROL_LOT: LotView = {
  lotId: "00000000-0000-7000-8000-000000003001",
  materialKind: "control",
  productName: "Synthetic control",
  lotNumber: "CTL-001",
  levelText: "Level 1",
  sampleLabel: "QC-L1",
  expiresOn: "2027-01-31",
  expired: false,
  state: "active",
  recordedAt: "2026-10-12T07:00:00Z",
  revision: 1,
};

export const REAGENT_LOT: LotView = {
  ...CONTROL_LOT,
  lotId: "00000000-0000-7000-8000-000000003002",
  materialKind: "reagent",
  productName: "Synthetic reagent",
  lotNumber: "RGT-009",
  levelText: null,
  sampleLabel: null,
  expiresOn: "2026-09-30",
  expired: true,
};

export const RUN_ID = "00000000-0000-7000-8000-000000004001";
export const METHOD_VERSION_ID = "00000000-0000-7000-8000-000000000d01";

export const RUN: RunDetail = {
  laboratoryProfileId: null,
  performingSiteSource: null,
  performingSiteReason: null,
  runId: RUN_ID,
  instrumentId: INSTRUMENT.instrumentId,
  instrumentName: INSTRUMENT.name,
  methodVersionId: METHOD_VERSION_ID,
  methodCode: "SYN-IA",
  methodVersion: 1,
  state: "open",
  createdBy: "analyst-1",
  createdAt: "2026-10-12T10:00:00Z",
  closedAt: null,
  closeReason: null,
  revision: 3,
  lots: [CONTROL_LOT],
  positions: [
    {
      patientContext: null,
      positionId: "position-1",
      label: "P01",
      role: "patient",
      sampleLabel: "C00000001",
      lotId: null,
      execution: {
        executionId: "execution-1",
        containerId: "container-1",
        attemptKind: "initial",
        relatedExecutionId: null,
        dilutionFactor: null,
        prepBatchId: null,
        reason: null,
        state: "active",
        stateReason: null,
        revision: 1,
        componentIds: ["component-1"],
        expectedAnalytes: 2,
        allowedActions: ["fail_execution"],
      },
    },
    {
      patientContext: null,
      positionId: "position-2",
      label: "Q01",
      role: "control",
      sampleLabel: "QC-L1",
      lotId: CONTROL_LOT.lotId,
      execution: null,
    },
  ],
  allowedActions: ["add_patient_position", "add_qc_position", "close_run"],
};

export const SUBJECT: SubjectSummary = {
  subjectId: "00000000-0000-7000-8000-000000005001",
  revision: 1,
  familyName: "Patient",
  givenNames: "Test",
  birthDate: "1980-01-01",
  administrativeSex: null,
  identifiers: [
    {
      id: "identifier-1",
      assigningAuthority: "SYN-LAB",
      identifierType: "MR",
      value: "MRN-0001",
      active: true,
      addedAt: "2026-10-12T08:00:00Z",
      deactivatedAt: null,
      deactivationReason: null,
    },
  ],
  openIdentityHolds: 0,
};

export function measurement(overrides: Partial<MeasurementView> = {}): MeasurementView {
  return {
    measurementId: "measurement-1",
    runId: RUN_ID,
    positionId: "position-1",
    positionLabel: "P01",
    attemptKind: "initial",
    processingRevisionSeq: 1,
    valueText: "512.0",
    unitsText: "ng/mL",
    statusText: "F",
    superseded: false,
    ...overrides,
  };
}

export function revision(overrides: Partial<ResultRevisionView> = {}): ResultRevisionView {
  return {
    revisionId: "revision-1",
    revisionNo: 1,
    cause: "initial",
    outcome: "quantity",
    comparator: null,
    magnitude: "512.0",
    units: "ng/mL",
    code: null,
    classification: "meets_cutoff",
    classificationReason: null,
    methodVersionId: METHOD_VERSION_ID,
    methodCertainty: "presumptive",
    outputScale: "quantitative",
    sequenceRole: "first_line",
    reason: null,
    createdBy: "system",
    createdAt: "2026-10-12T11:00:00Z",
    evidence: [measurement()],
    ...overrides,
  };
}
