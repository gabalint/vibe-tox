import type { MethodAnalyte } from "@/generated/MethodAnalyte";
import type { MethodVersionContent } from "@/generated/MethodVersionContent";
import type { MethodVersionView } from "@/generated/MethodVersionView";
import { checked, choiceOf, optionalText } from "@/lib/choices";
import { CERTAINTIES, OUTPUT_SCALES, THRESHOLD_ROUNDINGS, THRESHOLD_RULES } from "../labels";

/** One analyte row as typed. Limits are decimal text and stay text (REQ-ANA-002). */
export interface MethodAnalyteValues {
  analyteId: string | null;
  unit: string;
  cutoff: string;
  limitOfDetection: string;
  limitOfQuantitation: string;
  reportableLow: string;
  reportableHigh: string;
  displayScale: string;
  referenceInterval: string;
  rangePolicy: string | null;
}

/** The method version form. Names follow the engine's fields, so its errors land on them. */
export interface MethodFormValues {
  specimenTypeId: string | null;
  certainty: string | null;
  outputScale: string | null;
  thresholdRule: string | null;
  thresholdRounding: string | null;
  analytes: MethodAnalyteValues[];
  reportNotes: string;
}

export const EMPTY_ANALYTE: MethodAnalyteValues = {
  analyteId: null,
  unit: "",
  cutoff: "",
  limitOfDetection: "",
  limitOfQuantitation: "",
  reportableLow: "",
  reportableHigh: "",
  displayScale: "",
  referenceInterval: "",
  rangePolicy: "report_only",
};

/** Empty for a first draft; nothing is preselected. */
export const EMPTY_METHOD: MethodFormValues = {
  specimenTypeId: null,
  certainty: null,
  outputScale: null,
  thresholdRule: null,
  thresholdRounding: null,
  analytes: [EMPTY_ANALYTE],
  reportNotes: "",
};

/** A version's content as form values, to edit it or to start a new draft from it. */
export function methodValues(view: MethodVersionView): MethodFormValues {
  return {
    specimenTypeId: view.specimenTypeId,
    reportNotes: view.reportNotes ?? "",
    certainty: view.certainty,
    outputScale: view.outputScale,
    thresholdRule: view.thresholdRule,
    thresholdRounding: view.thresholdRounding,
    analytes: view.analytes.map((analyte) => ({
      analyteId: analyte.analyteId,
      referenceInterval: analyte.referenceInterval ?? "",
      rangePolicy: analyte.rangePolicy ?? "report_only",
      unit: analyte.unit ?? "",
      cutoff: analyte.cutoff ?? "",
      limitOfDetection: analyte.limitOfDetection ?? "",
      limitOfQuantitation: analyte.limitOfQuantitation ?? "",
      reportableLow: analyte.reportableLow ?? "",
      reportableHigh: analyte.reportableHigh ?? "",
      displayScale:
        analyte.displayScale === undefined || analyte.displayScale === null
          ? ""
          : String(analyte.displayScale),
    })),
  };
}

/** The version content from validated form values. Decimals are sent as typed. */
export function toMethodContent(values: MethodFormValues): MethodVersionContent {
  return {
    specimenTypeId: checked(values.specimenTypeId),
    reportNotes: optionalText(values.reportNotes),
    certainty: checked(choiceOf(CERTAINTIES, values.certainty)),
    outputScale: checked(choiceOf(OUTPUT_SCALES, values.outputScale)),
    thresholdRule: checked(choiceOf(THRESHOLD_RULES, values.thresholdRule)),
    thresholdRounding: checked(choiceOf(THRESHOLD_ROUNDINGS, values.thresholdRounding)),
    analytes: values.analytes.map(toAnalyte),
  };
}

function toAnalyte(analyte: MethodAnalyteValues): MethodAnalyte {
  const displayScale = optionalText(analyte.displayScale);
  return {
    analyteId: checked(analyte.analyteId),
    referenceInterval: optionalText(analyte.referenceInterval),
    rangePolicy: checked(choiceOf(RANGE_POLICIES, analyte.rangePolicy)),
    unit: optionalText(analyte.unit),
    cutoff: optionalText(analyte.cutoff),
    limitOfDetection: optionalText(analyte.limitOfDetection),
    limitOfQuantitation: optionalText(analyte.limitOfQuantitation),
    reportableLow: optionalText(analyte.reportableLow),
    reportableHigh: optionalText(analyte.reportableHigh),
    // A count of decimal places, not a clinical quantity; its format check accepted digits only.
    displayScale: displayScale === null ? null : Number.parseInt(displayScale, 10),
  };
}

export const RANGE_POLICIES = [
  { id: "report_only", label: "Report only" },
  { id: "require_within_range", label: "Require within range" },
] as const;
