import type { CommandError } from "@/generated/CommandError";

/** The engine's gate for a word-derived result on a dilution run (result files brief F7, F11). */
const GATE = "dilution_not_supported";

const WORDS =
  "A word such as N/F cannot be used on a dilution run; the method's limit is not the limit of a diluted injection.";

/**
 * A reviewer's refusal, with the gate for a value word on a dilution run said in plain words.
 * On results commands that gate means only this; other refusals pass through unchanged.
 */
export function explainWordOnDilution(error: CommandError | null): CommandError | null {
  if (!error?.gateReasons.some((gate) => gate.code === GATE)) return error;
  return {
    ...error,
    gateReasons: error.gateReasons.map((gate) =>
      gate.code === GATE ? { ...gate, message: WORDS } : gate,
    ),
  };
}
