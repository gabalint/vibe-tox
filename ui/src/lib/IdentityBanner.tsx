import { Link } from "react-router";
import { Banner } from "@/components/Banner";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { StatusBadge } from "@/components/StatusBadge";
import type { CommandName } from "@/generated/CommandName";
import type { SubjectIdentifierView } from "@/generated/SubjectIdentifierView";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import type { SubjectView } from "@/generated/SubjectView";
import { checked } from "./choices";
import { useCommandQuery, useHasCapability } from "./commands";
import { useFormatBirthDate } from "./format";
import styles from "./IdentityBanner.module.css";

/** `get_subject({ input: { subjectId } }) → SubjectView`: an audited read of the patient. */
const GET_SUBJECT = "get_subject" satisfies CommandName;

/** The capability the patient page's command (`get_subject`) needs. */
const OPEN_PATIENTS = "enter_orders";

/** The HL7 v2 identifier type code (table 0203) of a medical record number. */
const MEDICAL_RECORD_NUMBER = "MR";

/** The specimen a screen is about, on specimen screens. */
interface SpecimenIdentity {
  accessionNumber: string;
  /** The container's label code; `null` when it has none, left out when not relevant. */
  labelCode?: string | null;
}

type IdentityBannerProps = (
  | { subject: SubjectSummary; subjectId?: never }
  | { subjectId: string; subject?: never }
) & { specimen?: SpecimenIdentity };

/**
 * The current demographics (the last assertion), active identifiers and open hold count, from
 * a full subject record; the one conversion, also used by the front-desk screens.
 */
export function subjectSummary(subject: SubjectView): SubjectSummary {
  // Registration always records the first assertion, so a subject without one is a defect.
  const current = checked(subject.assertions.at(-1) ?? null);
  return {
    subjectId: subject.id,
    revision: subject.revision,
    familyName: current.familyName,
    givenNames: current.givenNames,
    birthDate: current.birthDate,
    administrativeSex: current.administrativeSex,
    identifiers: subject.identifiers.filter((identifier) => identifier.active),
    openIdentityHolds: subject.openHolds.length,
  };
}

/**
 * Who, and which specimen, a screen is about, kept in view while it scrolls: the family name
 * in capitals, the given names, the date of birth with its age, the active identifiers with
 * the medical record number in bold, and any identity hold or possible match (REQ-IDN-001).
 * Given `subjectId`, it loads the patient, which is an audited read: render it only on a
 * screen the person explicitly opened.
 */
export function IdentityBanner({ specimen, ...props }: IdentityBannerProps) {
  if (props.subject) return <SubjectBanner subject={props.subject} specimen={specimen} />;
  return <LoadedSubjectBanner subjectId={props.subjectId} specimen={specimen} />;
}

function LoadedSubjectBanner({
  subjectId,
  specimen,
}: {
  subjectId: string;
  specimen: SpecimenIdentity | undefined;
}) {
  const subject = useCommandQuery<SubjectView>(GET_SUBJECT, { input: { subjectId } });
  if (subject.isSuccess) {
    return <SubjectBanner subject={subjectSummary(subject.data)} specimen={specimen} />;
  }
  const value = subject.isError ? (
    <ErrorState
      title="The patient could not be loaded"
      error={subject.error}
      onRetry={() => void subject.refetch()}
    />
  ) : (
    <LoadingState label="Loading patient…" />
  );
  return (
    <Banner
      label={bannerLabel(specimen)}
      items={[{ label: "Patient", value }, ...specimenItems(specimen)]}
    />
  );
}

function SubjectBanner({
  subject,
  specimen,
}: {
  subject: SubjectSummary;
  specimen: SpecimenIdentity | undefined;
}) {
  const formatBirthDate = useFormatBirthDate();
  const holds = subject.openIdentityHolds;
  return (
    <Banner
      label={bannerLabel(specimen)}
      items={[
        { label: "Patient", value: <PatientName subject={subject} />, emphasis: true },
        { label: "Date of birth", value: formatBirthDate(subject.birthDate) },
        ...identifierItems(subject.identifiers),
        ...specimenItems(specimen),
      ]}
      status={
        holds > 0 && (
          <StatusBadge tone="hold">
            {holds === 1 ? "Identity hold" : `${holds} identity holds`}
          </StatusBadge>
        )
      }
    />
  );
}

/**
 * The patient's name, linking to the patient's page for sessions that may open it
 * (`enter_orders`, as the navigation shows Patients); plain text for everyone else, so a
 * reviewer is not offered a page the engine refuses. Presentation only: the engine decides.
 */
function PatientName({ subject }: { subject: SubjectSummary }) {
  const canOpenPatients = useHasCapability(OPEN_PATIENTS);
  const name = (
    <>
      <span className={styles.familyName}>{subject.familyName}</span>
      {subject.givenNames && `, ${subject.givenNames}`}
    </>
  );
  if (!canOpenPatients) return name;
  return <Link to={`/patients/${subject.subjectId}`}>{name}</Link>;
}

function identifierItems(identifiers: readonly SubjectIdentifierView[]) {
  if (identifiers.length === 0) return [{ label: "Identifiers", value: "None recorded" }];
  return identifiers.map(({ identifierType, assigningAuthority, value }) =>
    identifierType === MEDICAL_RECORD_NUMBER
      ? { label: `MRN (${assigningAuthority})`, value: <strong>{value}</strong> }
      : { label: `${identifierType} (${assigningAuthority})`, value },
  );
}

function specimenItems(specimen: SpecimenIdentity | undefined) {
  if (specimen === undefined) return [];
  const items = [{ label: "Accession", value: specimen.accessionNumber }];
  if (specimen.labelCode !== undefined) {
    items.push({ label: "Container", value: specimen.labelCode ?? "Not recorded" });
  }
  return items;
}

function bannerLabel(specimen: SpecimenIdentity | undefined): string {
  return specimen === undefined ? "Patient identity" : "Patient and specimen identity";
}
