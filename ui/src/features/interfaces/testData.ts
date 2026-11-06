// Synthetic Interfaces records for the feature's tests.
import type { MllpListener } from "@/generated/MllpListener";
import type { MllpPartner } from "@/generated/MllpPartner";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerMappings } from "@/generated/PartnerMappings";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SftpSettings } from "@/generated/SftpSettings";
import type { UnroutedMessage } from "@/generated/UnroutedMessage";

export const CONNECTION_ID = "01a1a000-0000-7000-8000-000000000001";
const ME = "01a1a000-0000-7000-8000-0000000000aa";

/** A synthetic partner server, as mockup 6 fills it. */
export const SFTP: SftpSettings = {
  host: "sftp.clinic-a.example",
  port: 22,
  username: "vibetox_lab",
  auth: "key",
  fingerprint: `SHA256:8Zq3${"A".repeat(35)}f1Kx`,
  remoteFolder: "/inbound/results",
};

export const ADMIN: SessionInfo = {
  userId: ME,
  username: "admin",
  displayName: "Test Admin",
  roles: [],
  capabilities: ["manage_configuration"],
  idleTimeoutSeconds: 900,
  locked: false,
};

export const CONNECTION: PartnerConnection = {
  id: CONNECTION_ID,
  settings: {
    name: "Billing - RCM",
    partner: "Synthetic billing company",
    messageType: "billing_json",
    protocol: "folder",
    rules: [{ field: "client", condition: "equals", value: "CLINIC-A" }],
    schedule: { kind: "immediately" },
    fileNamePattern: "billing-{handoff}_{date}.json",
    sftp: null,
    intake: null,
    mllp: null,
    mllpListener: null,
  },
  folder: "/srv/billing/out",
  processedFolder: null,
  failedFolder: null,
  hasPassword: false,
  active: false,
  revision: 1,
  status: "stopped",
  verification: null,
  lastAttempt: null,
  waiting: 1,
  failing: 0,
  awaitingAck: 0,
  listener: null,
  lastChange: { by: ME, at: "2026-11-03T10:00:00.000000000Z" },
  allowedActions: [
    "update_partner_connection",
    "set_partner_connection_active",
    "test_partner_folder",
    "send_partner_test_message",
    "record_partner_destination_check",
  ],
};

/** An HL7 Results connection, started and checked. */
export const HL7: PartnerConnection = {
  ...CONNECTION,
  settings: {
    ...CONNECTION.settings,
    name: "HL7 Results - Clinic A",
    partner: "CLINIC-A",
    messageType: "hl7_oru",
    rules: [{ field: "client", condition: "equals", value: "CLINIC-A" }],
    fileNamePattern: "{accession}_r{revision}_{date}.hl7",
  },
};

/** A synthetic partner interface on the lab network, as mockup 1 lists it. */
export const MLLP: MllpPartner = {
  address: "10.20.0.15",
  port: 6661,
  acknowledgement: "application",
  ackTimeoutSeconds: 30,
};

/** An HL7 Results connection the LIS sends over MLLP, with two messages awaiting an ACK. */
export const HL7_MLLP: PartnerConnection = {
  ...HL7,
  awaitingAck: 2,
  settings: {
    ...HL7.settings,
    name: "HL7 Results - Hospital B",
    partner: "CLINIC-B",
    protocol: "mllp_connect",
    fileNamePattern: "",
    mllp: MLLP,
  },
};

/** Where the LIS listens for the clinic's EMR, as mockup 2 fills it. */
export const LISTENER: MllpListener = {
  address: "192.168.1.20",
  port: 6660,
  allowedPartners: [{ address: "192.168.1.50", note: "Hospital B interface engine" }],
  acknowledgement: "application",
  characterSet: "utf-8",
  idleTimeoutSeconds: 300,
};

/** An HL7 Orders connection the LIS listens for over MLLP, listening and checked. */
export const ORDERS_LISTEN: PartnerConnection = {
  ...CONNECTION,
  active: true,
  status: "active",
  listener: { listening: true, problem: null, since: "2026-11-06T09:00:00.000000000Z" },
  settings: {
    ...CONNECTION.settings,
    name: "HL7 Orders - Hospital B",
    partner: "CLINIC-B",
    messageType: "hl7_orders",
    protocol: "mllp_listen",
    rules: [],
    fileNamePattern: "",
    intake: { checkEverySeconds: 30, acceptFrom: "HOSP-B", unknownPatient: "hold" },
    mllpListener: LISTENER,
  },
  folder: "",
};

const AMPHETAMINE = "01a1a000-0000-7000-8000-000000000051";
export const PHENTERMINE = "01a1a000-0000-7000-8000-000000000052";

/** One approved analyte mapping and one analyte a waiting message is held for. */
export const MAPPINGS: PartnerMappings = {
  connectionId: CONNECTION_ID,
  version: 2,
  approved: { by: ME, at: "2026-11-03T11:00:00.000000000Z" },
  rows: [
    {
      kind: "analyte",
      lisId: AMPHETAMINE,
      lisCode: "SYN-AMP",
      lisName: "Amphetamine",
      approved: { partnerCode: "CA-AMP", ucumUnit: "ng/mL" },
      draft: null,
      held: false,
    },
    {
      kind: "analyte",
      lisId: PHENTERMINE,
      lisCode: "SYN-PHEN",
      lisName: "Phentermine",
      approved: null,
      draft: null,
      held: true,
    },
  ],
  allowedActions: ["draft_partner_mapping"],
};

export const DETAIL: PartnerConnectionDetail = {
  connection: CONNECTION,
  history: [
    {
      revision: 1,
      change: "created",
      changed: { by: ME, at: "2026-11-03T10:00:00.000000000Z" },
      reason: null,
      fields: [],
    },
  ],
};

export const WRITTEN: PartnerMessageRow = {
  messageId: "01a1a000-0000-7000-8000-000000000010",
  deliveryId: "01a1a000-0000-7000-8000-000000000011",
  at: "2026-11-03T10:05:00.000000000Z",
  messageType: "billing_json",
  source: "billing_handoff",
  sourceId: "01a1a000-0000-7000-8000-000000000012",
  sourceNumber: 12,
  sourceLabel: null,
  connection: { id: CONNECTION_ID, name: "Billing - RCM", protocol: "folder" },
  reason: "routed",
  transport: "written",
  outcomeUnknown: true,
  waiting: null,
  attempts: 0,
  givenUp: false,
  superseded: false,
  nextAttemptAt: null,
  errorCode: null,
  writtenPath: "/srv/billing/out/billing-12_20261103.json",
  ackCode: null,
  ackText: null,
  awaitingAck: false,
  peer: null,
  unrouted: false,
  resolution: null,
  allowedActions: [],
  received: null,
};

export const FAILED: PartnerMessageRow = {
  ...WRITTEN,
  messageId: "01a1a000-0000-7000-8000-000000000020",
  deliveryId: "01a1a000-0000-7000-8000-000000000021",
  sourceNumber: 13,
  transport: "failed",
  outcomeUnknown: false,
  attempts: 5,
  givenUp: true,
  errorCode: "folder_missing",
  writtenPath: null,
  allowedActions: ["resend_partner_delivery"],
};

/** A corrected report revision sent first to a connection that had revision 1. */
export const CORRECTION: PartnerMessageRow = {
  ...WRITTEN,
  messageId: "01a1a000-0000-7000-8000-000000000040",
  deliveryId: "01a1a000-0000-7000-8000-000000000041",
  messageType: "report_pdf",
  source: "report_revision",
  sourceId: "01a1a000-0000-7000-8000-000000000042",
  sourceNumber: 2,
  sourceLabel: "A26000005-U",
  connection: { id: CONNECTION_ID, name: "Report PDFs - all clients", protocol: "folder" },
  reason: "follow_up",
  writtenPath: "/srv/reports/out/A26000005-U_r2_20261103.pdf",
};

export const UNROUTED: UnroutedMessage = {
  messageId: "01a1a000-0000-7000-8000-000000000030",
  createdAt: "2026-11-03T09:00:00.000000000Z",
  messageType: "billing_json",
  source: "billing_handoff",
  sourceId: "01a1a000-0000-7000-8000-000000000031",
  sourceNumber: 14,
  sourceLabel: null,
  facts: {
    clients: [{ value: "TEST-CLIENT-2", label: "TEST CLIENT 2" }],
    practitioners: [{ value: "1234567893", label: "Test, Doctor" }],
    locations: [],
    tests: [{ value: "SYN-OPIATES", label: "Synthetic opiates" }],
  },
  checkedAgainst: 1,
  closest: {
    connection: { id: CONNECTION_ID, name: "Billing - RCM", protocol: "folder" },
    rule: { field: "client", condition: "equals", value: "CLINIC-A" },
  },
  connections: [{ id: CONNECTION_ID, name: "Billing - RCM", protocol: "folder" }],
  allowedActions: ["resolve_unrouted_message"],
};
