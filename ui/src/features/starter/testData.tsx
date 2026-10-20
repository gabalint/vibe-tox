// Synthetic Standard Catalog data for the starter data tests. The codes,
// names and values are invented for the tests; they are not catalog content.
import type { StandardAnalyteItem } from "@/generated/StandardAnalyteItem";
import type { StandardCatalogStatus } from "@/generated/StandardCatalogStatus";
import type { StandardCatalogTree } from "@/generated/StandardCatalogTree";
import type { StandardItemState } from "@/generated/StandardItemState";
import type { StandardMethodItem } from "@/generated/StandardMethodItem";
import { mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { session } from "@/lib/test/session";
import { LOAD_CATALOG_PATH, STARTER_DATA_PATH } from "./contract";
import { LoadCatalogPage } from "./LoadCatalogPage";
import { StarterDataPage } from "./StarterDataPage";

export const DISCLAIMER =
  "Starter content: check every value before use. It is not validated for your laboratory and not ready for patient testing.";

const NEW = { state: "new", reason: null } as const;
const PRESENT = { state: "already_present", reason: null } as const;
const IN_USE = {
  state: "code_in_use",
  reason: "Your catalog uses this code with a different name.",
} as const;

function method(
  code: string,
  name: string,
  analytes: StandardMethodItem["analytes"],
  extra: Partial<StandardMethodItem> = {},
): StandardMethodItem {
  return {
    code,
    name,
    specimenTypeCode: "UR",
    certainty: "definitive",
    outputScale: "quantitative",
    thresholdRule: "at_or_above",
    thresholdRounding: "before_rounding",
    analytes,
    ...NEW,
    ...extra,
  };
}

function analyte(analyteCode: string, cutoff: string | null = null) {
  return {
    analyteCode,
    unit: "ng/mL",
    cutoff,
    cutoffSourceTitle: cutoff && "Synthetic guideline",
    cutoffLabel: cutoff && "Synthetic workplace cutoff",
  };
}

function analyteItem(
  code: string,
  name: string,
  status: { state: StandardItemState; reason: string | null },
): StandardAnalyteItem {
  return {
    code,
    name,
    drugClassCode: "OPI",
    drugClassName: "Opioids",
    loincCode: null,
    loincDisplayName: null,
    ...status,
  };
}

export const TREE: StandardCatalogTree = {
  info: {
    catalogVersion: "1.0.0",
    fileDigest: "ab12",
    reviewStatus: "unreviewed",
    disclaimer: DISCLAIMER,
    loincVersion: "2.83",
    loincNotice: "Synthetic LOINC notice for tests.",
  },
  specimenTypes: [
    { code: "UR", name: "Urine", ...NEW },
    { code: "OF", name: "Oral fluid", ...PRESENT },
  ],
  analytes: [
    analyteItem("UR-OPI-IA", "Opiates class", NEW),
    analyteItem("UR-AMP-IA", "Amphetamines class", NEW),
    analyteItem("UR-MORPH", "Morphine", NEW),
    analyteItem("UR-CODE", "Codeine", PRESENT),
    analyteItem("UR-6MAM", "6-Acetylmorphine", IN_USE),
    analyteItem("OF-MORPH", "Morphine (oral fluid)", PRESENT),
  ],
  methods: [
    method(
      "UR-IA",
      "Urine immunoassay screen",
      [analyte("UR-OPI-IA", "2000"), analyte("UR-AMP-IA")],
      {
        certainty: "presumptive",
        outputScale: "qualitative",
      },
    ),
    method("UR-OPI-MS", "Urine opioids LC-MS/MS", [
      analyte("UR-MORPH", "2000"),
      analyte("UR-CODE"),
      analyte("UR-6MAM"),
    ]),
    method("OF-OPI-MS", "Oral fluid opioids LC-MS/MS", [analyte("OF-MORPH")], {
      specimenTypeCode: "OF",
      ...PRESENT,
    }),
    method("UR-THC-MS", "Urine cannabinoids LC-MS/MS", [analyte("UR-MORPH")], IN_USE),
  ],
  orderables: [
    {
      code: "UR-SCREEN",
      name: "Urine screen panel",
      kind: "panel",
      methodCodes: ["UR-IA"],
      components: [
        { methodCode: "UR-IA", purpose: "analytical", analyteCodes: ["UR-OPI-IA", "UR-AMP-IA"] },
      ],
      ...NEW,
    },
    {
      code: "UR-OPI",
      name: "Urine opioids",
      kind: "test",
      methodCodes: ["UR-OPI-MS"],
      components: [
        { methodCode: "UR-OPI-MS", purpose: "analytical", analyteCodes: ["UR-MORPH", "UR-CODE"] },
      ],
      ...NEW,
    },
    {
      code: "OF-OPI",
      name: "Oral fluid opioids",
      kind: "test",
      methodCodes: ["OF-OPI-MS"],
      components: [{ methodCode: "OF-OPI-MS", purpose: "analytical", analyteCodes: ["OF-MORPH"] }],
      state: "blocked",
      reason: "Method already in your catalog; add this test by hand.",
    },
  ],
};

export const NOT_LOADED: StandardCatalogStatus = { loads: [], draftsRemaining: 0 };

export const LOADED: StandardCatalogStatus = {
  loads: [
    {
      id: "00000000-0000-7000-8000-00000000c001",
      catalogVersion: "1.0.0",
      fileDigest: "ab12",
      loadedBy: "00000000-0000-7000-8000-000000000002",
      loadedAt: "2026-10-13T08:00:00Z",
      records: 14,
    },
  ],
  draftsRemaining: 5,
};

type Handler = (args: unknown) => unknown;

/** Renders the starter data screens at `path` for a session holding `capabilities`. */
export function renderStarter(
  path: string,
  capabilities: readonly string[],
  commands: Record<string, Handler>,
) {
  mockCommands({ current_session: () => session(capabilities), ...commands });
  return renderRoutes(
    [
      { path: STARTER_DATA_PATH, element: <StarterDataPage /> },
      { path: LOAD_CATALOG_PATH, element: <LoadCatalogPage /> },
    ],
    path,
  );
}
