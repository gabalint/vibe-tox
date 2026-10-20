import type { OrderBillingView } from "@/generated/OrderBillingView";
import type { PatientBillingProfileView } from "@/generated/PatientBillingProfileView";
import type { PatientCoverage } from "@/generated/PatientCoverage";
import { ORDER_ID, SUBJECT_ID, SUMMARY } from "../testData";
import { emptyBillingContent } from "./PatientProfileDialog";

const COVERAGES: PatientCoverage[] = ["Synthetic Alpha", "Synthetic Beta"].map(
  (payerName, index) => ({
    key: `coverage-${index}`,
    payerName,
    payerId: null,
    planName: null,
    memberId: `000${index}`,
    groupNumber: null,
    subscriber: null,
    effectiveFrom: null,
    effectiveUntil: null,
    source: "Synthetic card",
    verifiedOn: null,
  }),
);
export const PROFILE: PatientBillingProfileView = {
  subjectId: SUBJECT_ID,
  subject: SUMMARY,
  allowedActions: ["set_patient_billing_profile"],
  profile: {
    subjectId: SUBJECT_ID,
    version: 4,
    content: { ...emptyBillingContent(), coverages: COVERAGES },
    source: "Synthetic intake",
    reason: "Supplied facts",
    recordedBy: "synthetic-user",
    recordedAt: "2026-10-14T12:00:00Z",
  },
};
export const BILLING: OrderBillingView = {
  orderId: ORDER_ID,
  subject: SUMMARY,
  orderRevision: 7,
  context: null,
  readinessIssues: ["Billing context missing"],
  reviewItems: [],
  allowedActions: [
    "set_order_billing_context",
    "create_billing_review_item",
    "update_billing_review_item",
  ],
};
