import type { DemographicsInput } from "@/generated/DemographicsInput";
import type { IdentifierInput } from "@/generated/IdentifierInput";
import type { SubjectMatchQuery } from "@/generated/SubjectMatchQuery";
import { optionalText } from "@/lib/choices";

/** The registration form as typed; every value is text until it is sent. */
export interface PatientDetailsValues {
  familyName: string;
  givenNames: string;
  birthDate: string;
  administrativeSex: string;
  source: string;
  identifiers: IdentifierInput[];
}

export const EMPTY_DETAILS: PatientDetailsValues = {
  familyName: "",
  givenNames: "",
  birthDate: "",
  administrativeSex: "",
  source: "",
  identifiers: [],
};

function toDemographics(values: PatientDetailsValues): DemographicsInput {
  return {
    familyName: values.familyName.trim(),
    givenNames: optionalText(values.givenNames),
    birthDate: values.birthDate.trim(),
    administrativeSex: optionalText(values.administrativeSex),
  };
}

export function toMatchQuery(values: PatientDetailsValues) {
  return {
    demographics: toDemographics(values),
    identifiers: values.identifiers.map(trimIdentifier),
  } satisfies SubjectMatchQuery;
}

export function trimIdentifier(identifier: IdentifierInput): IdentifierInput {
  return {
    assigningAuthority: identifier.assigningAuthority.trim(),
    identifierType: identifier.identifierType.trim(),
    value: identifier.value.trim(),
  };
}
