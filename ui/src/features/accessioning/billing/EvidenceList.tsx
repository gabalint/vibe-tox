import { Facts } from "@/components/Facts";
import { Timestamp } from "@/components/Timestamp";
import type { BillingNoticeEvidence } from "@/generated/BillingNoticeEvidence";
import type { FinancialEvidence } from "@/generated/FinancialEvidence";
import { labelOf } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import { EVIDENCE_KINDS, EVIDENCE_STATUSES } from "./evidenceValues";

export function EvidenceList({ evidence }: { evidence: FinancialEvidence[] }) {
  if (!evidence.length) return <p className={styles.hint}>No financial evidence recorded.</p>;
  return (
    <ul className={styles.list}>
      {evidence.map((item) => (
        <li key={item.key}>
          <details>
            <summary>
              {labelOf(EVIDENCE_KINDS, item.kind)} · {labelOf(EVIDENCE_STATUSES, item.status)} ·{" "}
              {item.scope}
            </summary>
            <Facts
              items={[
                { label: "Evidence reference", value: item.sourceRef },
                { label: "Event time", value: <Timestamp value={item.occurredAt} /> },
                { label: "Reference number", value: item.referenceNumber ?? "Not supplied" },
                {
                  label: "Validity",
                  value: `${item.validFrom ?? "Not supplied"} to ${item.validUntil ?? "Not supplied"}`,
                },
                { label: "Provider NPI", value: item.providerNpi ?? "Not supplied" },
                { label: "Units", value: item.units ?? "Not supplied" },
                { label: "Notes", value: item.notes ?? "None" },
              ]}
            />
            {item.notice && <NoticeFacts notice={item.notice} />}
          </details>
        </li>
      ))}
    </ul>
  );
}

function NoticeFacts({ notice }: { notice: BillingNoticeEvidence }) {
  return (
    <Facts
      items={[
        { label: "Form version", value: notice.formVersion ?? "Not supplied" },
        { label: "Notice reason", value: notice.reason },
        {
          label: "Estimated amount",
          value: `${notice.estimatedAmount ?? "Not supplied"} ${notice.currency ?? ""}`,
        },
        { label: "Patient choice", value: notice.choice ?? "Not supplied" },
        {
          label: "Provided at",
          value: notice.providedAt ? <Timestamp value={notice.providedAt} /> : "Not supplied",
        },
        {
          label: "Signed at",
          value: notice.signedAt ? <Timestamp value={notice.signedAt} /> : "Not supplied",
        },
        { label: "Signer", value: notice.signer ?? "Not supplied" },
      ]}
    />
  );
}
