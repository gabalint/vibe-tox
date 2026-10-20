import type { TextFormat } from "@/components/FormFields";

export const TIME_FORMAT: TextFormat = {
  pattern: /^([01]\d|2[0-3]):[0-5]\d$/,
  message: "Enter the time as HH:MM, 24-hour.",
  description: "HH:MM, 24-hour, on the collection site's clock",
};

export const OFFSET_FORMAT: TextFormat = {
  pattern: /^[+-](0\d|1[0-4]):[0-5]\d$/,
  message: "Enter the offset as ±HH:MM, such as -05:00.",
  description: "The site's offset from UTC, such as -05:00",
};

export const COUNT_FORMAT: TextFormat = {
  pattern: /^[1-9]\d*$/,
  message: "Enter a whole number of 1 or more.",
  description: "Containers received for this specimen",
};

export const VOLUME_FORMAT: TextFormat = {
  pattern: /^\d+(\.\d+)?$/,
  message: "Enter the volume as a number, such as 1.5.",
  description: "Exactly as measured or estimated",
};
