import { useFieldArray, useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { CheckList, ChoiceInput, RecordPickerInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { ClientView } from "@/generated/ClientView";
import type { OrderBillingView } from "@/generated/OrderBillingView";
import type { PatientBillingProfile } from "@/generated/PatientBillingProfile";
import type { PatientBillingProfileView } from "@/generated/PatientBillingProfileView";
import type { SetOrderBillingContextInput } from "@/generated/SetOrderBillingContextInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf, YES_NO } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { BILLING_RESPONSIBILITIES } from "../orders/labels";
import { FINANCIAL_QUERIES, GET_PATIENT_BILLING } from "./contract";
import { ChangeEvidenceFields } from "./FinancialFields";

interface Props {
  view: OrderBillingView;
  subjectId: string;
  onClose: () => void;
}
interface ContextValues {
  responsibility: string | null;
  clientId: string | null;
  profileVersion: string | null;
  coverages: { key: string | null }[];
  useGuarantor: string | null;
  evidenceKeys: string[];
  retainedEvidenceKeys: string[];
  source: string;
  reason: string;
}

export function OrderContextDialog(props: Props) {
  const [view] = useState(props.view);
  const current = useCommandQuery<PatientBillingProfileView>(GET_PATIENT_BILLING, {
    input: { subjectId: props.subjectId, version: null },
  });
  const priorVersion = view.context?.selection.profileVersion ?? null;
  const prior = useCommandQuery<PatientBillingProfileView>(GET_PATIENT_BILLING, {
    input: { subjectId: props.subjectId, version: priorVersion },
  });
  return (
    <QueryContent query={current} what="current patient billing profile">
      {(latest) => (
        <QueryContent query={prior} what="selected patient billing profile">
          {(previous) => {
            const profiles = [previous.profile, latest.profile].filter(
              (value): value is PatientBillingProfile => value !== null,
            );
            return (
              <ContextForm
                {...props}
                view={view}
                profiles={profiles.filter(
                  (p, index) => profiles.findIndex((each) => each.version === p.version) === index,
                )}
              />
            );
          }}
        </QueryContent>
      )}
    </QueryContent>
  );
}

function ContextForm({ view, profiles, onClose }: Props & { profiles: PatientBillingProfile[] }) {
  const selection = view.context?.selection;
  return (
    <CommandDialog<ContextValues, SetOrderBillingContextInput>
      title={view.context ? "Correct order billing selection" : "Confirm order billing selection"}
      submitLabel="Save order billing selection"
      command={{ name: "set_order_billing_context", invalidates: FINANCIAL_QUERIES }}
      defaultValues={{
        responsibility: selection?.responsibility ?? null,
        clientId: selection?.clientId ?? null,
        profileVersion: selection
          ? selection.profileVersion === null
            ? "none"
            : String(selection.profileVersion)
          : null,
        coverages: selection?.coverages.map((coverage) => ({ key: coverage.key })) ?? [],
        useGuarantor: selection ? (selection.guarantor ? "yes" : "no") : null,
        evidenceKeys: [],
        retainedEvidenceKeys: view.context?.financialEvidence.map((item) => item.key) ?? [],
        source: "",
        reason: "",
      }}
      toInput={(values) => {
        const profile = profiles.find((p) => String(p.version) === values.profileVersion);
        const retained =
          view.context?.financialEvidence.filter((item) =>
            values.retainedEvidenceKeys.includes(item.key),
          ) ?? [];
        const additions =
          profile?.content.financialEvidence.filter(
            (item) =>
              values.evidenceKeys.includes(item.key) &&
              !retained.some((prior) => prior.key === item.key),
          ) ?? [];
        return {
          orderId: view.orderId,
          expectedRevision: view.orderRevision,
          expectedBillingVersion: view.context?.version ?? null,
          responsibility: checked(choiceOf(BILLING_RESPONSIBILITIES, values.responsibility)),
          clientId: values.responsibility === "client" ? values.clientId : null,
          profileVersion: profile?.version ?? null,
          coverageKeys:
            values.responsibility === "third_party"
              ? values.coverages.map((item) => checked(item.key))
              : [],
          useProfileGuarantor: values.useGuarantor === "yes",
          financialEvidence: [...retained, ...additions],
          source: values.source.trim(),
          reason: values.reason.trim(),
        };
      }}
      successTitle="Order billing selection saved"
      onClose={onClose}
    >
      <p>
        The order keeps these selected facts. Patient profile changes require another explicit order
        correction.
      </p>
      <ResponsibilityField />
      <ClientField />
      <ProfileSelection profiles={profiles} />
      {view.context && view.context.financialEvidence.length > 0 && (
        <>
          <p>
            Deselect evidence that no longer applies to this order's billing selection. Earlier
            revisions retain the original evidence.
          </p>
          <CheckList
            name="retainedEvidenceKeys"
            legend="Order evidence retained in this revision"
            options={view.context.financialEvidence.map((item) => ({
              id: item.key,
              label: `${item.kind.replaceAll("_", " ")} — ${item.scope}`,
              details: [item.sourceRef, item.occurredAt],
            }))}
          />
        </>
      )}
      <ChangeEvidenceFields />
    </CommandDialog>
  );
}

function ResponsibilityField() {
  const { setValue } = useFormContext<ContextValues>();
  return (
    <ChoiceInput
      name="responsibility"
      label="Who is responsible for payment"
      options={BILLING_RESPONSIBILITIES}
      isRequired
      onChange={(value) => {
        if (value !== "third_party") setValue("coverages", [], { shouldDirty: true });
      }}
    />
  );
}

function ClientField() {
  const responsibility = useWatch({ name: "responsibility" });
  const clients = useCommandQuery<ClientView[]>("list_clients", undefined, {
    enabled: responsibility === "client",
  });
  return responsibility === "client" ? (
    <QueryContent query={clients} what="billing client accounts">
      {(items) => (
        <RecordPickerInput
          name="clientId"
          label="Bill-to client account"
          options={items.map((client) => ({ id: client.id, label: client.name }))}
          isRequired
        />
      )}
    </QueryContent>
  ) : null;
}

function ProfileSelection({ profiles }: { profiles: PatientBillingProfile[] }) {
  const { setValue } = useFormContext<ContextValues>();
  const version = useWatch<ContextValues, "profileVersion">({ name: "profileVersion" });
  const profile = profiles.find((p) => String(p.version) === version);
  const responsibility = useWatch({ name: "responsibility" });
  return (
    <>
      <ChoiceInput
        name="profileVersion"
        label="Patient financial facts to use"
        options={[
          { id: "none", label: "Do not select patient profile facts" },
          ...profiles.map((p) => ({
            id: String(p.version),
            label: `Profile revision ${p.version} — ${p.source}`,
          })),
        ]}
        isRequired
        onChange={() => {
          setValue("coverages", [], { shouldDirty: true });
          setValue("evidenceKeys", [], { shouldDirty: true });
          setValue("useGuarantor", null, { shouldDirty: true });
        }}
      />
      {profile && (
        <>
          {responsibility === "third_party" && <CoveragePriority profile={profile} />}
          {profile.content.guarantor ? (
            <ChoiceInput
              name="useGuarantor"
              label={`Use guarantor ${profile.content.guarantor.name}`}
              options={YES_NO}
              isRequired
            />
          ) : (
            <p className={styles.hint}>No guarantor in this profile revision.</p>
          )}
          {profile.content.financialEvidence.length > 0 && (
            <details>
              <summary>Select patient evidence for this order</summary>
              <CheckList
                name="evidenceKeys"
                legend="Evidence to copy"
                options={profile.content.financialEvidence.map((item) => ({
                  id: item.key,
                  label: `${item.kind.replaceAll("_", " ")} — ${item.scope}`,
                  details: [item.sourceRef, item.occurredAt],
                }))}
              />
            </details>
          )}
        </>
      )}
    </>
  );
}

function CoveragePriority({ profile }: { profile: PatientBillingProfile }) {
  const { fields, append, remove } = useFieldArray<ContextValues>({ name: "coverages" });
  return (
    <div className={styles.stack}>
      <p>
        Selected coverages, in payment priority. Select only coverages confirmed for this order.
      </p>
      {fields.map((field, index) => (
        <div key={field.id}>
          <RecordPickerInput
            name={`coverages.${index}.key`}
            label={`Coverage priority ${index + 1}`}
            options={profile.content.coverages.map((coverage) => ({
              id: coverage.key,
              label: coverage.payerName,
              details: [
                coverage.memberId ?? "No member identifier",
                coverage.planName ?? "No plan name",
              ],
            }))}
            isRequired
          />
          <Button onPress={() => remove(index)}>Remove coverage priority {index + 1}</Button>
        </div>
      ))}
      {profile.content.coverages.length > fields.length && (
        <Button onPress={() => append({ key: null })}>Add coverage to order</Button>
      )}
    </div>
  );
}

import { useState } from "react";
