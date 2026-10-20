import { useState } from "react";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { PatientBillingProfileView } from "@/generated/PatientBillingProfileView";
import type { PatientCoverage } from "@/generated/PatientCoverage";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { PartyFacts } from "./BillingFacts";
import { ContactDialog } from "./ContactDialog";
import { CoverageDialog } from "./CoverageDialog";
import { GET_PATIENT_BILLING } from "./contract";
import { EvidenceFields } from "./EvidenceFields";
import { EvidenceList } from "./EvidenceList";
import { EMPTY_EVIDENCE, evidenceInput } from "./evidenceValues";
import { GuarantorDialog } from "./GuarantorDialog";
import { PatientProfileDialog, type ProfileDialogProps } from "./PatientProfileDialog";
import { PatientProfileFacts } from "./PatientProfileFacts";

export function PatientBillingSection({ subjectId }: { subjectId: string }) {
  const [opened, setOpened] = useState(false);
  return (
    <Section title="Patient billing profile">
      <p className={styles.hint}>
        Reusable billing contact, responsible party, insurance and financial evidence. Orders retain
        their own confirmed selection.
      </p>
      {opened ? (
        <PatientBillingQuery subjectId={subjectId} />
      ) : (
        <Button onPress={() => setOpened(true)}>Open patient billing profile</Button>
      )}
    </Section>
  );
}

function PatientBillingQuery({ subjectId }: { subjectId: string }) {
  const query = useCommandQuery<PatientBillingProfileView>(GET_PATIENT_BILLING, {
    input: { subjectId, version: null },
  });
  return (
    <>
      <div>
        <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
          Refresh patient billing profile
        </Button>
      </div>
      <QueryContent query={query} what="patient billing profile">
        {(view) => <PatientBillingContent view={view} />}
      </QueryContent>
    </>
  );
}

function PatientBillingContent({ view }: { view: PatientBillingProfileView }) {
  const [dialog, setDialog] = useState<"contact" | "guarantor" | "coverage" | "evidence" | null>(
    null,
  );
  const [coverage, setCoverage] = useState<PatientCoverage | undefined>();
  const canEdit = view.allowedActions.includes("set_patient_billing_profile");
  const profile = view.profile;
  const content = profile?.content;
  const close = () => {
    setDialog(null);
    setCoverage(undefined);
  };
  const props = { view, onClose: close };
  return (
    <div className={styles.stack}>
      <PatientProfileFacts profile={profile} />
      {canEdit && (
        <div>
          <Button onPress={() => setDialog("contact")}>Edit billing contact</Button>{" "}
          <Button onPress={() => setDialog("guarantor")}>Edit guarantor</Button>
        </div>
      )}
      {content?.guarantor && (
        <details>
          <summary>Guarantor details</summary>
          <PartyFacts party={content.guarantor} />
        </details>
      )}
      <Section
        title="Patient coverages"
        variant="plain"
        actions={canEdit && <Button onPress={() => setDialog("coverage")}>Add coverage</Button>}
      >
        {!content?.coverages.length ? (
          <p>No coverage supplied.</p>
        ) : (
          <ul className={styles.list}>
            {content.coverages.map((item) => (
              <li key={item.key}>
                <details>
                  <summary>
                    {item.payerName} · {item.memberId ?? "No member identifier"}
                  </summary>
                  {item.subscriber && (
                    <details>
                      <summary>Subscriber details</summary>
                      <PartyFacts party={item.subscriber} />
                    </details>
                  )}
                  <Facts
                    items={[
                      { label: "Plan", value: item.planName ?? "Not supplied" },
                      { label: "Payer identifier", value: item.payerId ?? "Not supplied" },
                      { label: "Group", value: item.groupNumber ?? "Not supplied" },
                      {
                        label: "Subscriber",
                        value: item.subscriber
                          ? `${item.subscriber.name} (${item.subscriber.relationship})`
                          : "Not supplied",
                      },
                      {
                        label: "Coverage dates",
                        value: `${item.effectiveFrom ?? "Not supplied"} to ${item.effectiveUntil ?? "Not supplied"}`,
                      },
                      { label: "Source", value: item.source },
                      { label: "Checked on", value: item.verifiedOn ?? "Not supplied" },
                    ]}
                  />
                  {canEdit && (
                    <Button
                      onPress={() => {
                        setCoverage(item);
                        setDialog("coverage");
                      }}
                    >
                      Correct coverage
                    </Button>
                  )}
                </details>
              </li>
            ))}
          </ul>
        )}
      </Section>
      <Section
        title="Patient financial evidence"
        variant="plain"
        actions={
          canEdit && <Button onPress={() => setDialog("evidence")}>Add patient evidence</Button>
        }
      >
        <EvidenceList evidence={content?.financialEvidence ?? []} />
      </Section>
      {dialog === "contact" && <ContactDialog {...props} />}
      {dialog === "guarantor" && <GuarantorDialog {...props} />}
      {dialog === "coverage" && <CoverageDialog {...props} coverage={coverage} />}
      {dialog === "evidence" && <PatientEvidenceDialog {...props} />}
    </div>
  );
}

function PatientEvidenceDialog(props: ProfileDialogProps) {
  const [key] = useState(() => crypto.randomUUID());
  return (
    <PatientProfileDialog
      {...props}
      title="Add patient financial evidence"
      defaultValues={{ ...EMPTY_EVIDENCE, source: "", reason: "" }}
      update={(content, values) => ({
        ...content,
        financialEvidence: [...content.financialEvidence, evidenceInput(values, key)],
      })}
    >
      <EvidenceFields coverages={props.view.profile?.content.coverages ?? []} />
    </PatientProfileDialog>
  );
}
