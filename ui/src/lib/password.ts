// Fast feedback on new passwords. The engine applies the password policy again (REQ-IDN-004).

const MIN_LENGTH = 12;
const MAX_LENGTH = 128;

/** The length rule, shown beside every field that sets a password. */
export const PASSWORD_RULE = `Use ${MIN_LENGTH} to ${MAX_LENGTH} characters.`;

/** Presence and length of a new password, counted in characters rather than UTF-16 units. */
export function checkNewPassword(value: string): true | string {
  const length = [...value].length;
  if (length === 0) return "Enter a password.";
  if (length < MIN_LENGTH) return `Use at least ${MIN_LENGTH} characters.`;
  if (length > MAX_LENGTH) return `Use at most ${MAX_LENGTH} characters.`;
  return true;
}
