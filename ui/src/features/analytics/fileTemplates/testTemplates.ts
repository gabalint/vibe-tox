import type { FileLayout } from "@/generated/FileLayout";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { LayoutCheckRow } from "@/generated/LayoutCheckRow";
import type { LayoutSampleOpened } from "@/generated/LayoutSampleOpened";
import type { PreviewLine } from "@/generated/PreviewLine";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";

/**
 * Synthetic result-file exports and template answers for the template page tests, after the
 * illustrations in the result files brief §4. Not vendor files; no real laboratory data.
 */

export const SAMPLE_HANDLE = "00000000-0000-7000-8000-00000000f001";

const LONG_TEXT = [
  "Exported batch SYN-01",
  "Sample Name,Sample Type,Component,Final Conc,Units,Dilution",
  "C00000123,Unknown,Morphine,152.4,ng/mL,1",
  "C00000123,Unknown,Morphine-D6,,ng/mL,1",
  "C00000123,Unknown,Fentanyl,N/F,ng/mL,1",
  "QC-LOW-L1234,QC,Morphine,48.9,ng/mL,1",
  "CAL-1,Standard,Morphine,10.2,ng/mL,1",
];

export const LONG_SAMPLE: LayoutSampleOpened = {
  sampleHandle: SAMPLE_HANDLE,
  bomEncoding: null,
  validUtf8: true,
  lines: LONG_TEXT,
};

export const LONG_LINES: PreviewLine[] = LONG_TEXT.map((text, index) => ({
  line: index + 1,
  cells: text.split(","),
}));

export const LONG_HEADER = [LONG_TEXT[1]?.split(",") ?? []];

function row(path: string, cells: Partial<LayoutCheckRow>): LayoutCheckRow {
  return {
    recordPath: path,
    sampleLabel: null,
    analyteName: null,
    headerText: null,
    analyteId: null,
    notResult: false,
    typeWord: null,
    role: null,
    valueText: null,
    reading: "quantity",
    wordMeaning: null,
    ...cells,
  };
}

/** The long sample read with no mapping: every name unmapped and every word unread. */
export const LONG_READING: LayoutCheck = {
  failure: null,
  rows: [
    row("L3", {
      sampleLabel: "C00000123",
      analyteName: "Morphine",
      typeWord: "Unknown",
      valueText: "152.4",
    }),
    row("L4", {
      sampleLabel: "C00000123",
      analyteName: "Morphine-D6",
      typeWord: "Unknown",
      valueText: "",
      reading: "empty",
    }),
    row("L5", {
      sampleLabel: "C00000123",
      analyteName: "Fentanyl",
      typeWord: "Unknown",
      valueText: "N/F",
      reading: "unmapped_word",
    }),
    row("L6", {
      sampleLabel: "QC-LOW-L1234",
      analyteName: "Morphine",
      typeWord: "QC",
      valueText: "48.9",
    }),
    row("L7", {
      sampleLabel: "CAL-1",
      analyteName: "Morphine",
      typeWord: "Standard",
      valueText: "10.2",
    }),
  ],
  malformedRows: [],
  counts: {
    rows: 5,
    malformedRows: 0,
    notResults: 0,
    unmappedAnalytes: 5,
    untyped: 5,
    quantities: 3,
    censored: 0,
    words: 0,
    unmappedWords: 1,
    empty: 1,
    zeroOrNegative: 0,
    malformedValues: 0,
  },
};

const WIDE_TEXT = [
  "Sample,,,Morphine Results,,Fentanyl Results,,Qualifier (338.2 -> 105.1) Results",
  "Name,Type,Dil.,RT,Final Conc.,RT,Final Conc.,Ratio",
  "C00000123,Sample,1,2.31,152.4,,,",
  "QC-LOW-L1234,QC,1,2.30,48.9,4.12,1.98,",
];

export const WIDE_SAMPLE: LayoutSampleOpened = {
  sampleHandle: SAMPLE_HANDLE,
  bomEncoding: "utf8",
  validUtf8: true,
  lines: WIDE_TEXT,
};

export const WIDE_LINES: PreviewLine[] = WIDE_TEXT.map((text, index) => ({
  line: index + 1,
  cells: text.split(","),
}));

export const WIDE_HEADER = WIDE_TEXT.slice(0, 2).map((text) => text.split(","));

export const WIDE_READING: LayoutCheck = {
  ...LONG_READING,
  rows: [
    row("L3/Morphine Results", {
      sampleLabel: "C00000123",
      analyteName: "Morphine Results",
      valueText: "152.4",
    }),
    row("L3/Fentanyl Results", {
      sampleLabel: "C00000123",
      analyteName: "Fentanyl Results",
      valueText: "",
      reading: "empty",
    }),
  ],
  counts: { ...LONG_READING.counts, rows: 2, unmappedAnalytes: 2, unmappedWords: 0, quantities: 1 },
};

export const MORPHINE = {
  id: "00000000-0000-7000-8000-000000000b11",
  code: "SYN-MOR",
  name: "Synthetic morphine",
  loincCode: null,
  drugClassCode: null,
};

export const FENTANYL = {
  id: "00000000-0000-7000-8000-000000000b12",
  code: "SYN-FEN",
  name: "Synthetic fentanyl",
  loincCode: null,
  drugClassCode: null,
};

/** The long template as the page builds it from the steps the tests take. */
export const LONG_LAYOUT: FileLayout = {
  encoding: "utf8",
  delimiter: "comma",
  trimFields: true,
  header: LONG_HEADER,
  shape: { kind: "long", analyteColumn: 2, valueColumn: 3 },
  columns: {
    sampleLabel: 0,
    units: null,
    sampleType: 1,
    flags: null,
    operator: null,
    acquiredAt: null,
    dilution: null,
  },
  endOfData: "end_of_file",
  sampleTypes: [{ word: "Standard", role: "calibrator" }],
  valueWords: [{ word: "N/F", meaning: "below_loq" }],
  notResults: ["Morphine-D6"],
  declaredUnit: "ng/mL",
};

export const FILE_INSTRUMENT_ID = "00000000-0000-7000-8000-000000001003";

export const FILE_INSTRUMENT: InstrumentReadinessView = {
  instrumentId: FILE_INSTRUMENT_ID,
  name: "Synthetic LC-MS/MS 1",
  status: "in_service",
  since: "2026-10-13T08:00:00Z",
  transport: "result_file",
  activeRegistration: null,
};

export const FILE_SOURCE: SourceRegistrationView = {
  registrationId: "00000000-0000-7000-8000-000000002003",
  instrumentId: FILE_INSTRUMENT_ID,
  senderName: "Synthetic Quant",
  senderVersion: "12.1",
  sampleIdField: "specimen_id",
  revisionIndicator: "none",
  reportsFinalConcentration: true,
  state: "draft",
  authoredBy: "author-1",
  authoredAt: "2026-10-13T09:00:00Z",
  approvedBy: null,
  approvedAt: null,
  retiredAt: null,
  revision: 1,
  testCodes: [{ sourceTestCode: "Morphine", analyteId: MORPHINE.id }],
  fileLayout: LONG_LAYOUT,
  allowedActions: ["approve_source"],
};
