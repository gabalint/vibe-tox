import type { SubjectIdentifierView } from "@/generated/SubjectIdentifierView";

/** A patient's name as `Family, Given`, or the family name alone. */
export function subjectName(subject: { familyName: string; givenNames: string | null }): string {
  return subject.givenNames ? `${subject.familyName}, ${subject.givenNames}` : subject.familyName;
}

/** Identifiers as `value (authority type)`, so the scope of each value is always visible. */
export function identifierText(
  identifiers: readonly Pick<
    SubjectIdentifierView,
    "assigningAuthority" | "identifierType" | "value"
  >[],
): string {
  if (identifiers.length === 0) return "None recorded";
  return identifiers
    .map(
      ({ assigningAuthority, identifierType, value }) =>
        `${value} (${assigningAuthority} ${identifierType})`,
    )
    .join(", ");
}

/** A value the source may not have supplied, shown as such rather than left blank. */
export function orNotRecorded(value: string | null): string {
  return value ?? "Not recorded";
}

export function patientPath(subjectId: string): string {
  return `/patients/${subjectId}`;
}

/** Order entry for one patient. */
export function newOrderPath(subjectId: string): string {
  return `/orders/new?subjectId=${subjectId}`;
}
