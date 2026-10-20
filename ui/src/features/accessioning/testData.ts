import type { AccessionDetail } from "@/generated/AccessionDetail";
import type { ClientView } from "@/generated/ClientView";
import type { ContainerView } from "@/generated/ContainerView";
import type { OrderAccessionContext } from "@/generated/OrderAccessionContext";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import type { OrderDetail } from "@/generated/OrderDetail";
import type { PractitionerView } from "@/generated/PractitionerView";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import type { SubjectView } from "@/generated/SubjectView";

/** Synthetic front-desk records for the accessioning feature tests. No real people. */
export const SUBJECT_ID = "00000000-0000-7000-8000-000000000201";
export const OTHER_SUBJECT_ID = "00000000-0000-7000-8000-000000000202";

const MRN = {
  id: "00000000-0000-7000-8000-000000000301",
  assigningAuthority: "SYN-CLINIC",
  identifierType: "MR",
  value: "MRN-00001",
  active: true,
  addedAt: "2026-10-12T14:00:00Z",
  deactivatedAt: null,
  deactivationReason: null,
};

export const SUMMARY: SubjectSummary = {
  subjectId: SUBJECT_ID,
  revision: 1,
  familyName: "Patient",
  givenNames: "Test",
  birthDate: "1970-01-01",
  administrativeSex: "U",
  identifiers: [MRN],
  openIdentityHolds: 0,
};

export const OTHER_SUMMARY: SubjectSummary = {
  ...SUMMARY,
  subjectId: OTHER_SUBJECT_ID,
  identifiers: [],
};

export const SUBJECT_VIEW: SubjectView = {
  id: SUBJECT_ID,
  revision: 1,
  assertions: [
    {
      id: "00000000-0000-7000-8000-000000000401",
      sequence: 1,
      familyName: "Patient",
      givenNames: "Test",
      birthDate: "1970-01-01",
      administrativeSex: "U",
      source: "Paper requisition",
      reason: null,
      assertedAt: "2026-10-12T14:00:00Z",
      assertedByKind: "user",
      assertedById: "00000000-0000-7000-8000-000000000101",
    },
  ],
  identifiers: [MRN],
  openHolds: [],
};

export const HELD_SUBJECT_VIEW: SubjectView = {
  ...SUBJECT_VIEW,
  openHolds: [
    {
      id: "00000000-0000-7000-8000-000000000501",
      revision: 1,
      kind: "possible_duplicate",
      subjectId: SUBJECT_ID,
      candidateSubjectIds: [OTHER_SUBJECT_ID],
      openedAt: "2026-10-12T14:00:00Z",
    },
  ],
};

export function sessionWith(capabilities: string[]): SessionInfo {
  return {
    userId: "00000000-0000-7000-8000-000000000101",
    username: "frontdesk",
    displayName: "Test Accessioner",
    roles: [],
    capabilities,
    idleTimeoutSeconds: 900,
    locked: false,
  };
}

export const CLIENTS: ClientView[] = [
  {
    id: "00000000-0000-7000-8000-000000000601",
    active: true,
    revision: 1,
    code: "syn-clinic",
    name: "Synthetic Clinic",
    locations: [
      {
        active: true,
        revision: 1,
        id: "00000000-0000-7000-8000-000000000602",
        code: "north",
        name: "North Office",
      },
    ],
  },
];

export const PRACTITIONERS: PractitionerView[] = [
  {
    id: "00000000-0000-7000-8000-000000000701",
    active: true,
    revision: 1,
    npi: "0000000000",
    familyName: "Doctor",
    givenNames: "Test",
  },
];

function orderable(index: number, code: string, name: string): OrderableVersionView {
  return {
    id: `00000000-0000-7000-8000-00000000080${index}`,
    orderableId: `00000000-0000-7000-8000-00000000090${index}`,
    code,
    name,
    kind: "test",
    version: 1,
    components: [
      {
        position: 1,
        purpose: "analytical",
        methodVersionId: `00000000-0000-7000-8000-00000000100${index}`,
        methodCode: `m-${code}`,
        methodVersion: 1,
        specimenTypeCode: "urine",
        certainty: "presumptive",
        outputScale: "qualitative",
        analytes: [],
      },
    ],
    status: "approved",
    revision: 2,
    createdBy: "author",
    createdAt: "2026-10-12T09:00:00Z",
    approvedBy: "approver",
    effectiveFrom: "2026-10-12T10:00:00Z",
    effectiveTo: null,
    replacedBy: null,
    retiredBy: null,
    retiredAt: null,
    retireReason: null,
  };
}

export const SCREEN = orderable(1, "SYN-SCR", "Synthetic drug screen");
export const CONFIRM = orderable(2, "SYN-CNF", "Synthetic confirmation");
export const ORDERABLES = [SCREEN, CONFIRM];

/** A catalog longer than a plain checklist: 16 more synthetic tests after the two above. */
export const LONG_CATALOG = [
  ...ORDERABLES,
  ...Array.from({ length: 16 }, (_, index) =>
    orderable(index + 10, `SYN-T${index + 10}`, `Synthetic panel ${index + 10}`),
  ),
];

/** The worst-case catalog of order entry: 1,000 tests, the two above first. */
export const LARGE_CATALOG = [
  ...ORDERABLES,
  ...Array.from({ length: 998 }, (_, index) =>
    orderable(index + 10, `SYN-L${index + 10}`, `Synthetic large panel ${index + 10}`),
  ),
];

export const ORDER_ID = "00000000-0000-7000-8000-000000001101";
export const ITEM_ID = "00000000-0000-7000-8000-000000001201";

export const ORDER: OrderDetail = {
  orderId: ORDER_ID,
  version: 1,
  revision: 1,
  status: "received",
  subjectId: SUBJECT_ID,
  practitionerId: PRACTITIONERS[0]?.id ?? "",
  clientId: CLIENTS[0]?.id ?? "",
  clientLocationId: null,
  placerOrderId: "PO-0001",
  placerAuthority: "SYN-CLINIC",
  sourceKind: "paper_requisition",
  sourceDocumentRef: "REQ-SCAN-0001",
  billingResponsibility: "not_supplied",
  facilitySetting: null,
  standingOrderId: null,
  templateName: null,
  templateConfirmedPerPatient: null,
  receivedAt: "2026-10-12T14:05:00Z",
  items: [
    {
      itemId: ITEM_ID,
      kind: "initial",
      state: "requested",
      orderableVersionId: SCREEN.id,
      orderableCode: SCREEN.code,
      orderableName: SCREEN.name,
      sequenceRole: "first_line",
      parentItemId: null,
      reflexAuthorization: "authorized",
      reflexScopeOrderableIds: [CONFIRM.orderableId],
      authorityReference: null,
      reason: null,
      revision: 1,
      components: [
        {
          componentId: "00000000-0000-7000-8000-000000001251",
          catalogComponentPosition: 1,
          specimenTypeCode: "urine",
          purpose: "analytical",
          sequenceRole: "first_line",
          state: "active",
          revision: 1,
        },
      ],
    },
  ],
  evidence: {
    authority: [
      {
        evidenceId: "ev-1",
        kind: "wet_signature",
        documentRef: "REQ-SCAN-0001",
        addedInVersion: 1,
        removedInVersion: null,
      },
    ],
    diagnoses: [],
    medications: [
      {
        evidenceId: "ev-2",
        receivedText: "Synthetic medication A",
        code: null,
        codeSystem: null,
        source: "ordering_clinic",
        reporter: null,
        providedOn: null,
        asOfDate: null,
        addedInVersion: 1,
        removedInVersion: null,
      },
    ],
    pointOfCareResults: [],
  },
  holds: [],
  billingReviewItems: [],
  cancellations: [],
  triggerAssessments: [],
  allowedActions: ["amend_evidence", "accept", "reject", "place_hold"],
};

export function container(overrides: Partial<ContainerView> = {}): ContainerView {
  return {
    containerId: "00000000-0000-7000-8000-000000001401",
    labelCode: "C00000001",
    specimenId: "00000000-0000-7000-8000-000000001301",
    kind: "primary",
    parentContainerId: null,
    state: "available",
    volume: "30.0",
    volumeUnit: "mL",
    volumeKind: "estimated",
    location: null,
    locationChronologyUnresolved: false,
    openHolds: [],
    revision: 1,
    labelJobs: [],
    allowedActions: ["create_aliquot", "mark_container_depleted", "move_container", "print_labels"],
    ...overrides,
  };
}

export const ACCESSION_ID = "00000000-0000-7000-8000-000000001501";

/** Accepted, unallocated order material for the receiving task. */
export const ACCESSION_CONTEXT: OrderAccessionContext = {
  orderId: ORDER_ID,
  items: [
    {
      orderItemId: ITEM_ID,
      eligible: true,
      reason: null,
      accessionId: null,
      accessionNumber: null,
      requiredSpecimenTypeCodes: ["urine"],
    },
  ],
  accessions: [],
  allowedActions: ["create_accession"],
};

export function orderAccessionContext(overrides: Partial<OrderAccessionContext> = {}) {
  return { ...ACCESSION_CONTEXT, ...overrides };
}

export const ACCESSION: AccessionDetail = {
  accessionId: ACCESSION_ID,
  accessionNumber: "A26000001",
  orderId: ORDER_ID,
  orderItemIds: [ITEM_ID],
  subjectId: SUBJECT_ID,
  createdAt: "2026-10-12T15:00:00Z",
  collection: {
    collectionId: "00000000-0000-7000-8000-000000001601",
    precision: "datetime",
    collectedAt: "2026-10-12T13:30:00Z",
    utcOffsetMinutes: -300,
    collectedDate: null,
    collector: "Clinic nurse",
    observations: [],
  },
  specimens: [
    {
      specimenId: "00000000-0000-7000-8000-000000001301",
      specimenTypeCode: "urine",
      acceptance: "received",
      receivedAt: "2026-10-12T15:00:00Z",
      revision: 1,
      decisions: [],
      containers: [container()],
      allowedActions: ["record_receipt_decision"],
    },
  ],
};
