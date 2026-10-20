/** One option of a closed set of engine codes, with its label in laboratory terms. */
export interface Choice<Code extends string> {
  id: Code;
  label: string;
}

/** The label of `code`; an unknown code shows as received. */
export function labelOf<Code extends string>(choices: readonly Choice<Code>[], code: Code): string {
  return choices.find((choice) => choice.id === code)?.label ?? code;
}

/** Narrows a selected value to one of `choices`, or `null` when nothing valid is selected. */
export function choiceOf<Code extends string>(
  choices: readonly Choice<Code>[],
  value: unknown,
): Code | null {
  return choices.find((choice) => choice.id === value)?.id ?? null;
}

/**
 * A value the form's required rule has already checked. Reaching `null` here is a defect in
 * the form, not a user error, so it fails loudly instead of sending a guessed value.
 */
export function checked<T>(value: T | null): T {
  if (value === null) throw new Error("A required field was empty after validation.");
  return value;
}

/** Trimmed text, or `null` for a field left empty: optional values are never sent as "". */
export function optionalText(value: string): string | null {
  const text = value.trim();
  return text === "" ? null : text;
}

/** Trimmed text, or `undefined` to leave out an optional input field the user left empty. */
export function omittedIfBlank(value: string): string | undefined {
  return optionalText(value) ?? undefined;
}

/** Yes or no for facts the requisition states; neither is assumed. */
export const YES_NO: readonly Choice<"yes" | "no">[] = [
  { id: "yes", label: "Yes" },
  { id: "no", label: "No" },
];
