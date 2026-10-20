import type { EvidenceInput } from "@/generated/EvidenceInput";
import type { RegisterOrderInput } from "@/generated/RegisterOrderInput";
import type { TemplateUseInput } from "@/generated/TemplateUseInput";
import { checked, choiceOf, optionalText, YES_NO } from "@/lib/choices";
import {
  AUTHORITY_KINDS,
  BILLING_RESPONSIBILITIES,
  MEDICATION_SOURCES,
  REFLEX_AUTHORIZATIONS,
  SEQUENCE_ROLES,
  SOURCE_KINDS,
  TEMPLATE_KINDS,
} from "./labels";

/**
 * The order entry form as typed. Choices start empty and texts blank: every element is
 * entered as the requisition supplies it, never prefilled (REQ-ORD-005).
 */
export interface OrderFormValues {
  standingOrderId: string | null;
  clientId: string | null;
  clientLocationId: string | null;
  practitionerId: string | null;
  sourceKind: string | null;
  sourceDocumentRef: string;
  placerOrderId: string;
  placerAuthority: string;
  billingResponsibility: string | null;
  facilitySetting: string;
  templateKind: string | null;
  templateName: string;
  templateConfirmed: string | null;
  items: {
    orderableVersionId: string;
    sequenceRole: string | null;
    reflexAuthorization: string | null;
    reflexScopeOrderableIds: string[];
  }[];
  authority: { kind: string | null; documentRef: string };
  diagnoses: {
    code: string;
    codeSystem: string;
    codeSystemVersion: string;
    documentRef: string;
    preprinted: string | null;
  }[];
  medications: {
    receivedText: string;
    code: string;
    codeSystem: string;
    source: string | null;
    reporter: string;
    providedOn: string;
    asOfDate: string;
  }[];
  pointOfCareResults: {
    deviceOrTest: string;
    analyteText: string;
    resultText: string;
    performedAtText: string;
    source: string;
  }[];
}

export const EMPTY_ORDER: OrderFormValues = {
  standingOrderId: null,
  clientId: null,
  clientLocationId: null,
  practitionerId: null,
  sourceKind: null,
  sourceDocumentRef: "",
  placerOrderId: "",
  placerAuthority: "",
  billingResponsibility: null,
  facilitySetting: "",
  templateKind: null,
  templateName: "",
  templateConfirmed: null,
  items: [],
  authority: { kind: null, documentRef: "" },
  diagnoses: [],
  medications: [],
  pointOfCareResults: [],
};

/** Where the form's fields differ from `register_order`'s input, for its field errors. */
export const ORDER_FIELD_PATHS = {
  evidence: "",
  "templateUse.kind": "templateKind",
  "templateUse.name": "templateName",
  "templateUse.confirmedPerPatient": "templateConfirmed",
};

/** The `register_order` input from validated form values. */
export function toRegisterOrderInput(
  subjectId: string,
  values: OrderFormValues,
): RegisterOrderInput {
  return {
    subjectId,
    clientId: checked(values.clientId),
    clientLocationId: values.clientLocationId,
    practitionerId: checked(values.practitionerId),
    placerOrderId: optionalText(values.placerOrderId),
    placerAuthority: optionalText(values.placerAuthority),
    sourceKind: checked(choiceOf(SOURCE_KINDS, values.sourceKind)),
    sourceDocumentRef: optionalText(values.sourceDocumentRef),
    billingResponsibility: checked(
      choiceOf(BILLING_RESPONSIBILITIES, values.billingResponsibility),
    ),
    facilitySetting: optionalText(values.facilitySetting),
    standingOrderId: values.standingOrderId,
    templateUse: templateUse(values),
    items: values.items.map(toItem),
    evidence: toEvidence(values),
  };
}

function toItem(item: OrderFormValues["items"][number]): RegisterOrderInput["items"][number] {
  const reflexAuthorization = checked(choiceOf(REFLEX_AUTHORIZATIONS, item.reflexAuthorization));
  return {
    orderableVersionId: item.orderableVersionId,
    sequenceRole: checked(choiceOf(SEQUENCE_ROLES, item.sequenceRole)),
    reflexAuthorization,
    // A reflex scope means something only where reflex testing is authorized.
    reflexScopeOrderableIds:
      reflexAuthorization === "authorized" ? item.reflexScopeOrderableIds : [],
  };
}

export function toEvidence(values: OrderFormValues): EvidenceInput {
  return {
    authority: {
      kind: checked(choiceOf(AUTHORITY_KINDS, values.authority.kind)),
      documentRef: optionalText(values.authority.documentRef),
    },
    diagnoses: values.diagnoses.map((diagnosis) => ({
      code: diagnosis.code.trim(),
      codeSystem: diagnosis.codeSystem.trim(),
      codeSystemVersion: optionalText(diagnosis.codeSystemVersion),
      documentRef: diagnosis.documentRef.trim(),
      preprintedOnLabForm: checked(choiceOf(YES_NO, diagnosis.preprinted)) === "yes",
    })),
    medications: values.medications.map((medication) => ({
      receivedText: medication.receivedText.trim(),
      code: optionalText(medication.code),
      codeSystem: optionalText(medication.codeSystem),
      source: checked(choiceOf(MEDICATION_SOURCES, medication.source)),
      reporter: optionalText(medication.reporter),
      providedOn: optionalText(medication.providedOn),
      asOfDate: optionalText(medication.asOfDate),
    })),
    pointOfCareResults: values.pointOfCareResults.map((result) => ({
      deviceOrTest: result.deviceOrTest.trim(),
      analyteText: result.analyteText.trim(),
      // REQ-ORD-007: kept exactly as written; context only, never a laboratory result.
      resultText: result.resultText,
      performedAtText: optionalText(result.performedAtText),
      source: result.source.trim(),
    })),
  };
}

function templateUse(values: OrderFormValues): TemplateUseInput {
  if (checked(choiceOf(TEMPLATE_KINDS, values.templateKind)) === "none") return { kind: "none" };
  return {
    kind: "clinic_template",
    name: values.templateName.trim(),
    confirmedPerPatient: checked(choiceOf(YES_NO, values.templateConfirmed)) === "yes",
  };
}
