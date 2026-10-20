import { describe, expect, it } from "vitest";
import {
  alsoRemovedFrom,
  type Choices,
  chooseMethod,
  chooseOrderable,
  deriveSelection,
  keepAnalyte,
  NO_CHOICES,
  removalBlock,
  toLoadInput,
  unavailableText,
} from "./selection";
import { TREE } from "./testData";

function methodOf(code: string) {
  const found = TREE.methods.find((m) => m.code === code);
  if (!found) throw new Error(`no method ${code}`);
  return found;
}

describe("Standard Catalog selection (proposed REQ-OPS-014)", () => {
  it("implies a chosen panel's methods, and their specimen types and analytes", () => {
    const choices = chooseOrderable(TREE, NO_CHOICES, "UR-SCREEN", true);
    const selection = deriveSelection(TREE, choices);
    expect([...selection.methods]).toEqual(["UR-IA"]);
    expect(selection.methodNeeders.get("UR-IA")).toEqual(["Urine screen panel"]);
    expect(selection.specimenTypeNeeders.get("UR")).toEqual(["Urine immunoassay screen"]);
    expect([...selection.analyteNeeders.keys()]).toEqual(["UR-OPI-IA", "UR-AMP-IA"]);
    expect(selection.counts).toEqual({
      specimenTypes: 1,
      analytes: 2,
      methods: 1,
      tests: 0,
      panels: 1,
      skipped: 0,
    });
    expect(selection.blockers).toEqual([]);
  });

  it("releases implied items when the parent is cleared, keeping explicit choices", () => {
    let choices = chooseMethod(TREE, NO_CHOICES, "UR-IA", true);
    choices = chooseOrderable(TREE, choices, "UR-SCREEN", true);
    choices = chooseOrderable(TREE, choices, "UR-SCREEN", false);
    const selection = deriveSelection(TREE, choices);
    expect([...selection.methods]).toEqual(["UR-IA"]);
    expect(selection.methodNeeders.size).toBe(0);

    const cleared = deriveSelection(TREE, chooseMethod(TREE, choices, "UR-IA", false));
    expect(cleared.methods.size).toBe(0);
    expect(cleared.analyteNeeders.size).toBe(0);
    expect(cleared.specimenTypeNeeders.size).toBe(0);
  });

  it("counts present analytes as skipped and lists unavailable ones as blockers", () => {
    const choices = chooseMethod(TREE, NO_CHOICES, "UR-OPI-MS", true);
    const selection = deriveSelection(TREE, choices);
    expect(selection.counts.analytes).toBe(1);
    expect(selection.counts.skipped).toBe(1);
    expect(selection.blockers).toEqual([
      {
        code: "UR-6MAM",
        name: "6-Acetylmorphine",
        reason: "Your catalog uses this code with a different name.",
      },
    ]);
    // Removing the unavailable analyte clears the blocker.
    const removed = keepAnalyte(choices, "UR-OPI-MS", "UR-6MAM", false);
    expect(deriveSelection(TREE, removed).blockers).toEqual([]);
  });

  it("removes an analyte from the method and from every chosen component using it", () => {
    let choices = chooseOrderable(TREE, NO_CHOICES, "UR-OPI", true);
    expect(alsoRemovedFrom(TREE, choices, "UR-OPI-MS", "UR-CODE")).toEqual(["Urine opioids"]);
    expect(alsoRemovedFrom(TREE, choices, "UR-OPI-MS", "UR-6MAM")).toEqual([]);
    choices = keepAnalyte(choices, "UR-OPI-MS", "UR-CODE", false);
    const selection = deriveSelection(TREE, choices);
    expect(selection.analyteNeeders.has("UR-CODE")).toBe(false);
    expect(toLoadInput(TREE, choices, true).omittedAnalytes).toEqual([
      { methodCode: "UR-OPI-MS", analyteCode: "UR-CODE" },
    ]);
  });

  it("prevents removing a method's last analyte, or a chosen component's last one", () => {
    const screen = methodOf("UR-IA");
    let choices: Choices = chooseMethod(TREE, NO_CHOICES, "UR-IA", true);
    expect(removalBlock(TREE, choices, screen, "UR-OPI-IA")).toBeNull();
    choices = keepAnalyte(choices, "UR-IA", "UR-OPI-IA", false);
    expect(removalBlock(TREE, choices, screen, "UR-AMP-IA")).toBe(
      "Urine immunoassay screen needs at least one analyte; this is its last one.",
    );

    const opioids = methodOf("UR-OPI-MS");
    let test = chooseOrderable(TREE, NO_CHOICES, "UR-OPI", true);
    test = keepAnalyte(test, "UR-OPI-MS", "UR-CODE", false);
    expect(removalBlock(TREE, test, opioids, "UR-MORPH")).toBe(
      "Urine opioids needs at least one analyte from Urine opioids LC-MS/MS; this is its last one.",
    );
    expect(removalBlock(TREE, test, opioids, "UR-6MAM")).toBeNull();
  });

  it("forgets removals of a method that is no longer selected", () => {
    let choices = chooseMethod(TREE, NO_CHOICES, "UR-IA", true);
    choices = keepAnalyte(choices, "UR-IA", "UR-AMP-IA", false);
    choices = chooseMethod(TREE, choices, "UR-IA", false);
    expect(choices.omitted.size).toBe(0);
  });

  it("sends only the explicit choices, in file order", () => {
    let choices = chooseOrderable(TREE, NO_CHOICES, "UR-OPI", true);
    choices = chooseMethod(TREE, choices, "UR-IA", true);
    choices = chooseOrderable(TREE, choices, "UR-SCREEN", true);
    expect(toLoadInput(TREE, choices, true)).toEqual({
      catalogVersion: "1.0.0",
      orderableCodes: ["UR-SCREEN", "UR-OPI"],
      methodCodes: ["UR-IA"],
      omittedAnalytes: [],
      disclaimerAcknowledged: true,
    });
  });

  it("says why an item cannot be chosen", () => {
    expect(unavailableText({ state: "new", reason: null })).toBeNull();
    expect(unavailableText({ state: "already_present", reason: null })).toBe(
      "Already in your catalog",
    );
    expect(unavailableText({ state: "code_in_use", reason: "Different name." })).toBe(
      "Different name.",
    );
    expect(unavailableText({ state: "blocked", reason: null })).toBe("Cannot be loaded");
  });
});
