import { useState } from "react";
import { Navigate, useSearchParams } from "react-router";
import { Page, PageHeader } from "@/components/Page";
import type { CommandError } from "@/generated/CommandError";
import { UnsavedChangesGuard } from "@/lib/UnsavedChangesGuard";
import { newOrderPath, patientPath } from "../subjects";
import { CandidateReview } from "./CandidateReview";
import { PatientDetailsForm } from "./PatientDetailsForm";
import { EMPTY_DETAILS, type PatientDetailsValues } from "./registration";

/**
 * Registers a patient in two steps (REQ-IDN-001): enter the details, then review the existing
 * patients that match before registering. Changing the details checks again. Leaving with
 * typed details asks first.
 */
export function RegisterPatientPage() {
  const [params] = useSearchParams();
  const forOrder = params.get("intent") === "new-order";
  const returnPath = forOrder ? "/orders/new" : "/patients";
  const [draft, setDraft] = useState<PatientDetailsValues>(EMPTY_DETAILS);
  const [checking, setChecking] = useState(false);
  const [refusal, setRefusal] = useState<CommandError | null>(null);
  const [registeredId, setRegisteredId] = useState<string | null>(null);
  return (
    <Page>
      <PageHeader
        title="Register patient"
        breadcrumbs={[
          { label: forOrder ? "New order" : "Patients", href: returnPath },
          { label: "Register patient", href: "/patients/register" },
        ]}
        description="Enter the details exactly as the requisition gives them. Existing patients with the same identifiers, or the same name and birth date, are shown before anything is saved."
      />
      {checking ? (
        <>
          {/* Before the redirect, so the guard is released when the patient's page opens. */}
          <UnsavedChangesGuard when={registeredId === null} />
          {registeredId !== null && (
            <Navigate to={forOrder ? newOrderPath(registeredId) : patientPath(registeredId)} />
          )}
          <CandidateReview
            details={draft}
            onEdit={(error) => {
              setRefusal(error);
              setChecking(false);
            }}
            onRegistered={setRegisteredId}
            onChoose={forOrder ? (subject) => setRegisteredId(subject.subjectId) : undefined}
          />
        </>
      ) : (
        <PatientDetailsForm
          defaultValues={draft}
          hasDraft={draft !== EMPTY_DETAILS}
          error={refusal}
          returnPath={returnPath}
          onCheck={(values) => {
            setDraft(values);
            setRefusal(null);
            setChecking(true);
          }}
        />
      )}
    </Page>
  );
}
