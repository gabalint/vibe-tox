import { describe, expect, it } from "vitest";
import { checked, choiceOf, labelOf, omittedIfBlank, optionalText } from "./choices";

const KINDS = [
  { id: "test", label: "Test" },
  { id: "panel", label: "Panel" },
] as const;

describe("choices", () => {
  it("narrows a selection to a known code, and labels codes", () => {
    expect(choiceOf(KINDS, "panel")).toBe("panel");
    expect(choiceOf(KINDS, "other")).toBeNull();
    expect(labelOf(KINDS, "test")).toBe("Test");
  });

  it("never sends an empty optional text", () => {
    expect(optionalText("  ")).toBeNull();
    expect(optionalText(" mL ")).toBe("mL");
    expect(omittedIfBlank("")).toBeUndefined();
    expect(omittedIfBlank(" 0.50 ")).toBe("0.50");
  });

  it("fails loudly instead of sending a guessed value", () => {
    expect(() => checked(null)).toThrow();
    expect(checked("urine")).toBe("urine");
  });
});
