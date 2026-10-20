import type { TextFormat } from "@/components/FormFields";

/** A lexical decimal: sent as typed, never converted to a number (REQ-ANA-002). */
export const DECIMAL_FORMAT: TextFormat = {
  pattern: /^-?\d+(\.\d+)?$/,
  message: "Enter a decimal number, such as 0.50.",
  description: "Exactly as the laboratory set it, such as 0.50",
};

export const DISPLAY_SCALE_FORMAT: TextFormat = {
  pattern: /^\d{1,2}$/,
  message: "Enter a whole number of decimal places.",
  description: "Decimal places shown on results",
};

export const NPI_FORMAT: TextFormat = {
  pattern: /^\d{10}$/,
  message: "Enter the 10-digit NPI.",
  description: "10 digits",
};

export const WHOLE_NUMBER_FORMAT: TextFormat = {
  pattern: /^\d+$/,
  message: "Enter a whole number.",
  description: "A whole number",
};
