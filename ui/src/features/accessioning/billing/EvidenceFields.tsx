import { useWatch } from "react-hook-form";
import { FieldGrid } from "@/components/Form";
import {
  ChoiceInput,
  DATE_FORMAT,
  NPI_FORMAT,
  RecordPickerInput,
  TextInput,
} from "@/components/FormFields";
import type { PatientCoverage } from "@/generated/PatientCoverage";
import { EVIDENCE_KINDS, EVIDENCE_STATUSES } from "./evidenceValues";

export function EvidenceFields({ coverages }: { coverages: PatientCoverage[] }) {
  const kind = useWatch({ name: "kind" });
  const noticeStatus = useWatch({ name: "noticeStatus" });
  return (
    <>
      <p>
        Record the source's decision and actual event times. Eligibility or authorization does not
        guarantee payment.
      </p>
      <FieldGrid>
        <ChoiceInput name="kind" label="Evidence type" options={EVIDENCE_KINDS} isRequired />
        <ChoiceInput name="status" label="Source status" options={EVIDENCE_STATUSES} isRequired />
        <TextInput name="sourceRef" label="Evidence reference" isRequired />
        <TextInput name="scope" label="Services and circumstances covered" isRequired />
        <TextInput
          name="occurredAt"
          label="Source event time (UTC)"
          description="As documented, for example 2026-10-14T14:30:00Z."
          isRequired
        />
        <TextInput name="referenceNumber" label="Authorization or reference number" />
        <TextInput name="validFrom" label="Valid from" format={DATE_FORMAT} />
        <TextInput name="validUntil" label="Valid until" format={DATE_FORMAT} />
        <RecordPickerInput
          name="coverageKey"
          label="Related coverage"
          options={coverages.map((coverage) => ({
            id: coverage.key,
            label: `${coverage.payerName} — ${coverage.memberId ?? "No member identifier"}`,
          }))}
        />
        <TextInput name="providerNpi" label="Related provider NPI" format={NPI_FORMAT} />
        <TextInput
          name="units"
          label="Authorized units"
          format={{
            pattern: /^\d+$/,
            message: "Enter a whole number.",
            description: "Whole units, as authorized.",
          }}
        />
      </FieldGrid>
      {(kind === "advance_notice" || kind === "good_faith_estimate") && (
        <>
          <ChoiceInput
            name="noticeStatus"
            label="Notice or estimate details"
            options={[
              { id: "supplied", label: "Notice details supplied" },
              { id: "not_supplied", label: "Not supplied" },
            ]}
            isRequired
          />
          {noticeStatus === "supplied" && <NoticeFields />}
        </>
      )}
      <TextInput name="notes" label="Evidence notes" />
    </>
  );
}

function NoticeFields() {
  return (
    <FieldGrid>
      <TextInput name="notice.formVersion" label="Notice or estimate form version" />
      <TextInput name="notice.reason" label="Notice or estimate reason" isRequired />
      <TextInput
        name="notice.estimatedAmount"
        label="Estimated amount"
        description="Exact amount as supplied, without a currency symbol."
      />
      <TextInput name="notice.currency" label="Currency" />
      <TextInput name="notice.choice" label="Patient choice as documented" />
      <TextInput name="notice.providedAt" label="Provided to patient at (UTC)" />
      <TextInput name="notice.signedAt" label="Signed at (UTC)" />
      <TextInput name="notice.signer" label="Signer as documented" />
    </FieldGrid>
  );
}
