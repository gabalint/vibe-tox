import { describe, expect, it } from "vitest";
import { engineError } from "@/lib/test/ipc";
import { explainWordOnDilution } from "./wordOnDilution";

describe("a value word refused on a dilution run (result files brief F7, F11)", () => {
  it("says the gate in plain words and keeps the rest of the refusal", () => {
    const refusal = engineError("gate_blocked", "This action is blocked.", {
      gateReasons: [
        { code: "dilution_not_supported", message: "A value word does not apply to a dilution." },
        { code: "qc_pending", message: "QC not assessed." },
      ],
    });
    expect(explainWordOnDilution(refusal)?.gateReasons).toEqual([
      {
        code: "dilution_not_supported",
        message:
          "A word such as N/F cannot be used on a dilution run; the method's limit is not the limit of a diluted injection.",
      },
      { code: "qc_pending", message: "QC not assessed." },
    ]);
    expect(explainWordOnDilution(refusal)?.correlationId).toBe("corr-test-0001");
  });

  it("leaves other refusals unchanged", () => {
    const conflict = engineError("conflict", "Someone else changed this record.");
    expect(explainWordOnDilution(conflict)).toBe(conflict);
    expect(explainWordOnDilution(null)).toBeNull();
  });
});
