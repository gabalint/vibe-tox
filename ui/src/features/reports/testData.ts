import type { SessionInfo } from "@/generated/SessionInfo";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import type { ToxReportData } from "@/generated/ToxReportData";
import type {
  CandidateView,
  DestinationView,
  ObligationView,
  ReportDetail,
  ReportWorkspace,
  RevisionSummary,
} from "./contract";

/** Synthetic report records for the reports tests. No real patient or laboratory data. */

export const SPECIMEN_ID = "00000000-0000-7000-8000-000000006001";
export const CANDIDATE_ID = "00000000-0000-7000-8000-00000000c001";
export const REPORT_ID = "00000000-0000-7000-8000-00000000r001";
export const REVISION_ID = "00000000-0000-7000-8000-00000000e001";

export function sessionWith(capabilities: string[]): SessionInfo {
  return {
    userId: "00000000-0000-7000-8000-000000000e02",
    username: "releaser",
    displayName: "Test Releaser",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

const SUBJECT: SubjectSummary = {
  subjectId: "00000000-0000-7000-8000-000000005001",
  revision: 1,
  familyName: "Testpatient",
  givenNames: "Jane",
  birthDate: "1980-01-01",
  administrativeSex: null,
  identifiers: [
    {
      id: "00000000-0000-7000-8000-000000005101",
      assigningAuthority: "SYN",
      identifierType: "MRN",
      value: "MRN-00000",
      active: true,
      addedAt: "2026-10-12T08:00:00Z",
      deactivatedAt: null,
      deactivationReason: null,
    },
  ],
  openIdentityHolds: 0,
};

export const CONTENT: ToxReportData = {
  report: {
    number: "A26000001-URN",
    revision: "1",
    completeness: "partial",
    relationship: "initial",
    supersedes: null,
    amendmentReason: null,
    issuedAt: "2026-10-12 09:30 CDT",
    generatedAt: "2026-10-12 09:30 CDT",
    changeSummary: [],
  },
  laboratory: {
    name: "Synthetic Toxicology Laboratory",
    addressLines: ["1 Example Way", "Springfield, ZZ 00000"],
    cliaId: "00D0000000",
    director: "Dr. Test Director",
    phone: "555-0100",
  },
  subject: {
    name: "Testpatient, Jane",
    birthDate: "1980-01-01",
    sex: null,
    identifiers: [{ type: "MRN", value: "MRN-00000", authority: "SYN" }],
  },
  order: {
    practitioner: { name: "Dr. Synthetic Orderer", npi: "0000000000" },
    client: "Synthetic Clinic",
    location: null,
    placerOrder: "PO-0001",
  },
  specimen: {
    accession: "A26000001",
    matrix: "Urine",
    containerLabels: ["A26000001-1"],
    collected: "2026-10-12 08:00 CDT",
    received: null,
    conditionNotes: [],
    collectionObservations: [],
  },
  medications: [{ name: "Synthetic medication", source: "Requisition", asOf: null }],
  resultGroups: [
    {
      name: "Synthetic confirmation panel",
      method: "Synthetic LC-MS/MS v2",
      certainty: "Definitive",
      role: "Confirmation",
      reportNotes: null,
      rows: [
        {
          analyte: "Synthetic amphetamine",
          result: "512.4",
          units: "ng/mL",
          limits: "Cutoff 50 ng/mL; LOQ 5 ng/mL",
          comparison: "Meets cutoff",
          recordedAt: "2026-10-12 08:30 CDT",
          firstReportedAt: null,
          referenceInterval: null,
          performingSites: ["Synthetic Toxicology Laboratory; 1 Example Way; CLIA 00D0000000"],
        },
        {
          analyte: "Synthetic methamphetamine",
          result: "<5.0",
          units: "ng/mL",
          limits: "Cutoff 50 ng/mL; LOQ 5 ng/mL",
          comparison: "Does not meet cutoff",
          recordedAt: "2026-10-12 08:30 CDT",
          firstReportedAt: null,
          referenceInterval: null,
          performingSites: ["Synthetic Toxicology Laboratory; 1 Example Way; CLIA 00D0000000"],
        },
      ],
    },
  ],
  otherTests: [{ name: "Synthetic opiate panel", status: "pending", note: "" }],
  validity: { outcome: "Within expected range", notes: [] },
  releasedBy: { name: "Test Releaser" },
};

const RELEASER = { userId: "00000000-0000-7000-8000-000000000e02", displayName: "Test Releaser" };

export const CANDIDATE: CandidateView = {
  candidateId: CANDIDATE_ID,
  revision: 3,
  state: "ready",
  reportId: REPORT_ID,
  reportNumber: "A26000001-URN",
  proposedRevisionNo: 1,
  relationship: "initial",
  completeness: "partial",
  supersedesRevisionNo: null,
  amendmentReason: null,
  contentSha256: "c0ffee00".repeat(8),
  artifactSha256: "a1".repeat(32),
  preparedAt: "2026-10-12T14:30:00Z",
  expiresAt: "2026-10-12T15:30:00Z",
  signatory: RELEASER,
  content: CONTENT,
  dispositions: [
    {
      componentId: "component-1",
      name: "Synthetic confirmation panel",
      disposition: "reported",
      gates: [],
    },
    {
      componentId: "component-2",
      name: "Synthetic opiate panel",
      disposition: "pending",
      gates: [{ code: "review_stale:qc_rejected", message: "QC for this run was rejected." }],
    },
  ],
  gates: [],
  stale: false,
  staleReasons: [],
  allowedActions: ["release_report"],
};

export const REVISION: RevisionSummary = {
  revisionId: REVISION_ID,
  revisionNo: 1,
  relationship: "initial",
  completeness: "partial",
  availability: "current",
  releasedAt: "2026-10-12T14:40:00Z",
  releasedBy: RELEASER,
  amendmentReason: null,
  artifactSha256: "a1".repeat(32),
};

export const WORKSPACE: ReportWorkspace = {
  unavailableCandidateId: null,
  conditionReviewSource: { sourceSha256: "0".repeat(64), latestDecisionId: null, conditions: [] },
  specimenId: SPECIMEN_ID,
  report: null,
  subject: SUBJECT,
  accessionNumber: "A26000001",
  specimenTypeCode: "urine",
  preview: {
    content: CONTENT,
    dispositions: CANDIDATE.dispositions,
    completeness: "partial",
    relationship: "initial",
    gates: [],
  },
  openCandidate: null,
  allowedActions: ["prepare_report_candidate"],
};

const CLIENT_ID = "00000000-0000-7000-8000-000000000601";

export const OBLIGATION: ObligationView = {
  obligationId: "00000000-0000-7000-8000-00000000b001",
  clientId: CLIENT_ID,
  reportId: REPORT_ID,
  revisionId: REVISION_ID,
  reportNumber: "A26000001-URN",
  revisionNo: 1,
  kind: "report",
  destination: {
    destinationId: "00000000-0000-7000-8000-00000000d001",
    channel: "fax",
    label: "Synthetic Clinic fax",
    verified: true,
  },
  status: { code: "awaiting", label: "Fax by staff: awaiting delivery" },
  attempts: [],
  origin: "release",
  originReason: null,
  createdAt: "2026-10-12T14:40:00Z",
  allowedActions: ["record_delivery_attempt", "redirect_delivery_obligation", "export_report_pdf"],
};

export const REPORT: ReportDetail = {
  reportId: REPORT_ID,
  reportNumber: "A26000001-URN",
  specimenId: SPECIMEN_ID,
  clientId: CLIENT_ID,
  subject: SUBJECT,
  accessionNumber: "A26000001",
  availability: "current",
  revisions: [REVISION],
  obligations: [OBLIGATION],
  exports: [],
  impact: null,
  withdrawal: null,
  incidents: [],
  allowedActions: ["prepare_report_candidate", "withdraw_report", "add_delivery_obligation"],
};

export const DESTINATION: DestinationView = {
  destinationId: "00000000-0000-7000-8000-00000000d002",
  clientId: CLIENT_ID,
  clientLocationId: null,
  channel: "email",
  address: "results@clinic.example",
  label: "Results email",
  verified: {
    by: { userId: "00000000-0000-7000-8000-000000000e09", displayName: "Test Verifier" },
    at: "2026-10-12T09:00:00Z",
    method: "Test email confirmed by phone",
    byCreator: false,
  },
  state: "verified",
  retired: false,
  retireReason: null,
  replacesId: null,
  createdBy: { userId: "00000000-0000-7000-8000-000000000e08", displayName: "Test Configurer" },
  revision: 2,
  allowedActions: ["replace_delivery_destination", "retire_delivery_destination"],
};
