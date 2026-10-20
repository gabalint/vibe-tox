import type { FinancialEvidence } from "@/generated/FinancialEvidence";
import type { FinancialEvidenceKind } from "@/generated/FinancialEvidenceKind";
import type { FinancialEvidenceStatus } from "@/generated/FinancialEvidenceStatus";
import { type Choice, checked, choiceOf, optionalText } from "@/lib/choices";

export const EVIDENCE_KINDS: readonly Choice<FinancialEvidenceKind>[] = [
  { id: "eligibility", label: "Eligibility" },
  { id: "prior_authorization", label: "Prior authorization" },
  { id: "advance_notice", label: "Advance notice (including ABN)" },
  { id: "good_faith_estimate", label: "Good faith estimate" },
  { id: "provider_enrollment", label: "Provider enrollment" },
];
export const EVIDENCE_STATUSES: readonly Choice<FinancialEvidenceStatus>[] = [
  { id: "not_checked", label: "Not checked" },
  { id: "unknown", label: "Unknown" },
  { id: "not_required", label: "Not required" },
  { id: "pending", label: "Pending" },
  { id: "recorded", label: "Recorded" },
  { id: "approved", label: "Approved" },
  { id: "denied", label: "Denied" },
];
export const EMPTY_EVIDENCE = {
  kind: null as string | null,
  status: null as string | null,
  sourceRef: "",
  scope: "",
  occurredAt: "",
  validFrom: "",
  validUntil: "",
  referenceNumber: "",
  coverageKey: null as string | null,
  providerNpi: "",
  units: "",
  notes: "",
  noticeStatus: null as string | null,
  notice: {
    formVersion: "",
    reason: "",
    estimatedAmount: "",
    currency: "",
    choice: "",
    providedAt: "",
    signedAt: "",
    signer: "",
  },
};
export type EvidenceValues = typeof EMPTY_EVIDENCE;

export function evidenceInput(values: EvidenceValues, key: string): FinancialEvidence {
  const kind = checked(choiceOf(EVIDENCE_KINDS, values.kind));
  const notice = values.notice;
  return {
    key,
    kind,
    status: checked(choiceOf(EVIDENCE_STATUSES, values.status)),
    sourceRef: values.sourceRef.trim(),
    scope: values.scope.trim(),
    occurredAt: values.occurredAt.trim(),
    validFrom: optionalText(values.validFrom),
    validUntil: optionalText(values.validUntil),
    referenceNumber: optionalText(values.referenceNumber),
    coverageKey: values.coverageKey,
    providerNpi: optionalText(values.providerNpi),
    units: values.units.trim() ? Number(values.units) : null,
    notes: optionalText(values.notes),
    notice:
      (kind === "advance_notice" || kind === "good_faith_estimate") &&
      values.noticeStatus === "supplied"
        ? {
            formVersion: optionalText(notice.formVersion),
            reason: notice.reason.trim(),
            estimatedAmount: optionalText(notice.estimatedAmount),
            currency: optionalText(notice.currency),
            choice: optionalText(notice.choice),
            providedAt: optionalText(notice.providedAt),
            signedAt: optionalText(notice.signedAt),
            signer: optionalText(notice.signer),
          }
        : null,
  };
}
