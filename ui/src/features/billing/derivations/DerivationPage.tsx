import { useState } from "react";
import { Link, useParams } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { DerivationView } from "@/generated/DerivationView";
import { CommandConfirmDialog } from "@/lib/CommandConfirmDialog";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { IdentityBanner } from "@/lib/IdentityBanner";
import { useRecordSequence } from "@/lib/recordSequence";
import styles from "@/styles/layout.module.css";
import { DERIVATION_QUERIES, GET_SERVICE_DERIVATION } from "../contract";
import { DeriveDialog } from "../DeriveDialog";
import { COMPLETENESS, DOS_RULES, derivationState } from "../labels";
import { classMappingPath, derivationPath } from "../paths";
import { ContextResolutionDialog } from "./ContextResolutionDialog";
import { FlagsSection } from "./FlagsSection";
import { SpecimenSection } from "./SpecimenSection";

/** Where the breadcrumb leads when the derivation was not opened from a list. */
const HOME = { label: "Billing work", href: "/billing/work" };

/**
 * One service derivation for review (REQ-BIL-005): the patient and date of service, the
 * mapping version used, the flags, and per specimen the billing classes ordered, performed
 * and reported with each component's disposition. Opening it is recorded in the audit trail.
 */
export function DerivationPage() {
  const { derivationId = "" } = useParams();
  const derivation = useCommandQuery<DerivationView>(GET_SERVICE_DERIVATION, {
    input: { derivationId },
  });
  const { list, sequence } = useRecordSequence(derivationId, HOME, derivationPath);
  const data = derivation.data;
  const title = data ? `Service derivation ${data.derivationNo}` : "Service derivation";
  // One header in every state, so Previous and Next keep focus while the next one loads.
  return (
    <Page>
      <PageHeader
        title={title}
        breadcrumbs={[list, { label: title, href: derivationPath(derivationId) }]}
        sequence={sequence}
        actions={data && <DerivationActions derivation={data} />}
      />
      {data && <IdentityBanner subject={data.subject} />}
      <QueryContent query={derivation} what="derivation">
        {(loaded) => <DerivationContent derivation={loaded} />}
      </QueryContent>
    </Page>
  );
}

function DerivationContent({ derivation }: { derivation: DerivationView }) {
  const state = derivationState(derivation.state);
  return (
    <div className={styles.stack}>
      {derivation.stale && (
        <Alert tone="warning" title="Out of date">
          The work this derivation was built from has changed. Derive again before approval.
        </Alert>
      )}
      <Section title="Derivation">
        <Facts
          layout="compact"
          items={[
            { label: "State", value: <StatusBadge tone={state.tone}>{state.label}</StatusBadge> },
            { label: "Completeness", value: labelOf(COMPLETENESS, derivation.completeness) },
            { label: "Date of service", value: derivation.dateOfService ?? "Unknown" },
            { label: "Date of service rule", value: labelOf(DOS_RULES, derivation.dosRule) },
            {
              label: "Class mapping",
              value: derivation.mappingVersion ? (
                <Link to={classMappingPath(derivation.mappingVersion.version)}>
                  {`Version ${derivation.mappingVersion.version}`}
                </Link>
              ) : (
                "None"
              ),
            },
            ...adjustmentFacts(derivation),
            ...(derivation.contextResolution
              ? [
                  {
                    label: "Billing context reconciliation",
                    value: derivation.contextResolution.reason,
                  },
                ]
              : []),
          ]}
        />
      </Section>
      <FlagsSection flags={derivation.flags} references={derivation.flagReferenceLabels} />
      {derivation.specimens.map((specimen) => (
        <SpecimenSection key={specimen.specimenId} specimen={specimen} />
      ))}
    </div>
  );
}

function adjustmentFacts({ adjustmentOf }: DerivationView) {
  if (adjustmentOf === null) return [];
  return [
    {
      label: "Adjusts",
      value: (
        <Link to={derivationPath(adjustmentOf.derivationId)}>
          {`Derivation ${adjustmentOf.derivationNo}`}
        </Link>
      ),
    },
  ];
}

function DerivationActions({ derivation }: { derivation: DerivationView }) {
  const [dialog, setDialog] = useState<"approve" | "derive" | "context" | null>(null);
  // Views name the actions the engine allows by their command names.
  const allowed = (command: string) => derivation.allowedActions.includes(command);
  const close = () => setDialog(null);
  return (
    <>
      {allowed("resolve_billing_context_conflict") && (
        <Button onPress={() => setDialog("context")}>Resolve billing context conflict</Button>
      )}
      {dialog === "context" && <ContextResolutionDialog derivation={derivation} onClose={close} />}
      {allowed("approve_service_derivation") && (
        <Button variant="primary" onPress={() => setDialog("approve")}>
          Approve derivation
        </Button>
      )}
      {allowed("derive_services") && (
        <Button onPress={() => setDialog("derive")}>Derive again</Button>
      )}
      {dialog === "approve" && (
        <CommandConfirmDialog
          title={`Approve service derivation ${derivation.derivationNo}?`}
          confirmLabel="Approve derivation"
          variant="primary"
          command={{ name: "approve_service_derivation", invalidates: DERIVATION_QUERIES }}
          input={{ derivationId: derivation.derivationId, expectedRevision: derivation.revision }}
          successTitle={`Derivation ${derivation.derivationNo} approved`}
          onClose={close}
        >
          <p>
            You approve exactly these services for handoff to the billing partner. If the work has
            changed since, the engine refuses and the derivation must be derived again.
          </p>
        </CommandConfirmDialog>
      )}
      {dialog === "derive" && (
        <DeriveDialog
          input={{
            subjectId: derivation.subject.subjectId,
            dateOfService: derivation.dateOfService,
          }}
          onClose={close}
        />
      )}
    </>
  );
}
