// Synthetic Interfaces records for the feature's tests.
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { UnroutedMessage } from "@/generated/UnroutedMessage";

export const CONNECTION_ID = "01a1a000-0000-7000-8000-000000000001";
const ME = "01a1a000-0000-7000-8000-0000000000aa";

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
  },
  folder: "/srv/billing/out",
  active: false,
  revision: 1,
  status: "stopped",
  verification: null,
  lastAttempt: null,
  waiting: 1,
  failing: 0,
  lastChange: { by: ME, at: "2026-11-03T10:00:00.000000000Z" },
  allowedActions: [
    "update_partner_connection",
    "set_partner_connection_active",
    "test_partner_folder",
    "send_partner_test_message",
    "record_partner_destination_check",
  ],
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
  connection: { id: CONNECTION_ID, name: "Billing - RCM" },
  reason: "routed",
  transport: "written",
  outcomeUnknown: true,
  waiting: null,
  attempts: 0,
  givenUp: false,
  nextAttemptAt: null,
  errorCode: null,
  writtenPath: "/srv/billing/out/billing-12_20261103.json",
  unrouted: false,
  resolution: null,
  allowedActions: [],
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

export const UNROUTED: UnroutedMessage = {
  messageId: "01a1a000-0000-7000-8000-000000000030",
  createdAt: "2026-11-03T09:00:00.000000000Z",
  messageType: "billing_json",
  source: "billing_handoff",
  sourceId: "01a1a000-0000-7000-8000-000000000031",
  sourceNumber: 14,
  facts: {
    clients: [{ value: "TEST-CLIENT-2", label: "TEST CLIENT 2" }],
    practitioners: [{ value: "1234567893", label: "Test, Doctor" }],
    locations: [],
    tests: [{ value: "SYN-OPIATES", label: "Synthetic opiates" }],
  },
  checkedAgainst: 1,
  closest: {
    connection: { id: CONNECTION_ID, name: "Billing - RCM" },
    rule: { field: "client", condition: "equals", value: "CLINIC-A" },
  },
  connections: [{ id: CONNECTION_ID, name: "Billing - RCM" }],
  allowedActions: ["resolve_unrouted_message"],
};
