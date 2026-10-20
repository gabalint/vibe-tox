import { createContext, useCallback, useContext } from "react";

/**
 * The laboratory's IANA time zone from its profile, or `null` before a profile exists (or
 * while it cannot be read). Provided once by the app frame; without a provider, times show
 * in UTC.
 */
export const LabTimeZoneContext = createContext<string | null>(null);

interface TimeOptions {
  /** Adds seconds, where events within one minute must be told apart (the audit trail). */
  seconds?: boolean;
  /** Leaves out the date when it is the same day as `today` in the zone: `14:03 CDT`. */
  today?: Date;
}

const formatters = new Map<string, Intl.DateTimeFormat>();

function formatterFor(timeZone: string, seconds: boolean): Intl.DateTimeFormat {
  const key = `${timeZone}|${seconds}`;
  let formatter = formatters.get(key);
  if (formatter === undefined) {
    formatter = new Intl.DateTimeFormat("en-US", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: seconds ? "2-digit" : undefined,
      hourCycle: "h23",
      timeZoneName: "short",
    });
    formatters.set(key, formatter);
  }
  return formatter;
}

function zonedParts(date: Date, timeZone: string, seconds: boolean) {
  const parts = formatterFor(timeZone, seconds).formatToParts(date);
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((each) => each.type === type)?.value ?? "";
  const time = [part("hour"), part("minute"), ...(seconds ? [part("second")] : [])].join(":");
  return {
    day: `${part("year")}-${part("month")}-${part("day")}`,
    timeAndZone: `${time} ${part("timeZoneName")}`,
  };
}

function zonedText(date: Date, timeZone: string, { seconds = false, today }: TimeOptions): string {
  const { day, timeAndZone } = zonedParts(date, timeZone, seconds);
  if (today !== undefined && zonedParts(today, timeZone, seconds).day === day) return timeAndZone;
  return `${day} ${timeAndZone}`;
}

/**
 * An engine timestamp (RFC 3339) as `2026-10-12 09:03 CDT` in `timeZone`, always naming the
 * zone so it is never ambiguous. A zone this web engine does not know falls back to UTC; a
 * value that does not parse is shown as received.
 */
export function formatTimestamp(
  timestamp: string,
  timeZone: string | null,
  options: TimeOptions = {},
): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;
  try {
    return zonedText(date, timeZone ?? "UTC", options);
  } catch {
    return zonedText(date, "UTC", options);
  }
}

/** Formats engine timestamps in the laboratory time zone (UTC until a profile exists). */
export function useFormatTime(): (timestamp: string, options?: TimeOptions) => string {
  const timeZone = useContext(LabTimeZoneContext);
  return useCallback(
    (timestamp: string, options?: TimeOptions) => formatTimestamp(timestamp, timeZone, options),
    [timeZone],
  );
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

interface CalendarDate {
  year: number;
  month: number;
  day: number;
}

/** A `YYYY-MM-DD` calendar date, or `null` when it is not a real date. */
function parseCalendarDate(text: string): CalendarDate | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
  if (!match) return null;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  // Date.UTC rolls 2026-02-30 over to March; a real date survives the round trip.
  const check = new Date(Date.UTC(year, month - 1, day));
  if (check.getUTCMonth() !== month - 1 || check.getUTCDate() !== day) return null;
  return { year, month, day };
}

/** Today's calendar date in `timeZone` (UTC without one, or for a zone this engine lacks). */
function calendarToday(now: Date, timeZone: string | null): CalendarDate | null {
  let day: string;
  try {
    day = zonedParts(now, timeZone ?? "UTC", false).day;
  } catch {
    day = zonedParts(now, "UTC", false).day;
  }
  return parseCalendarDate(day);
}

/**
 * Completed days under 1 month, months under 2 years, then years, because neonatal specimens
 * exist; `null` for a birth date after today.
 */
function ageText(birth: CalendarDate, today: CalendarDate): string | null {
  const months =
    (today.year - birth.year) * 12 + (today.month - birth.month) - (today.day < birth.day ? 1 : 0);
  if (months < 0) return null;
  if (months >= 24) return `${Math.floor(months / 12)} y`;
  if (months >= 1) return `${months} mo`;
  const days =
    (Date.UTC(today.year, today.month - 1, today.day) -
      Date.UTC(birth.year, birth.month - 1, birth.day)) /
    86_400_000;
  return `${days} d`;
}

/**
 * A birth date (`YYYY-MM-DD`, a calendar date never shifted by a zone) with the month as
 * text and the age on today's date in the laboratory zone: `Nov 9, 1961 (64 y)`. A date
 * after today shows without an age; a value that does not parse is shown as received.
 */
export function formatBirthDate(birthDate: string, now: Date, timeZone: string | null): string {
  const birth = parseCalendarDate(birthDate);
  if (birth === null) return birthDate;
  const date = `${MONTHS[birth.month - 1]} ${birth.day}, ${birth.year}`;
  const today = calendarToday(now, timeZone);
  const age = today === null ? null : ageText(birth, today);
  return age === null ? date : `${date} (${age})`;
}

/** Formats birth dates with the age counted to today in the laboratory time zone. */
export function useFormatBirthDate(): (birthDate: string) => string {
  const timeZone = useContext(LabTimeZoneContext);
  return useCallback(
    (birthDate: string) => formatBirthDate(birthDate, new Date(), timeZone),
    [timeZone],
  );
}
