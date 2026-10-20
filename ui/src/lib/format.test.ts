import { describe, expect, it } from "vitest";
import { formatBirthDate, formatTimestamp } from "./format";

describe("formatTimestamp", () => {
  it("formats in the laboratory zone and names it, across daylight saving", () => {
    expect(formatTimestamp("2026-10-12T14:03:05Z", "America/Chicago")).toBe("2026-10-12 09:03 CDT");
    expect(formatTimestamp("2026-12-01T14:03:05Z", "America/Chicago")).toBe("2026-12-01 08:03 CST");
  });

  it("uses UTC, named, without a laboratory zone", () => {
    expect(formatTimestamp("2026-10-12T00:30:00Z", null)).toBe("2026-10-12 00:30 UTC");
  });

  it("falls back to UTC for a zone the web engine does not know", () => {
    expect(formatTimestamp("2026-10-12T14:03:05Z", "Not/AZone")).toBe("2026-10-12 14:03 UTC");
  });

  it("shows a value that does not parse as received", () => {
    expect(formatTimestamp("unknown", "America/Chicago")).toBe("unknown");
  });

  it("leaves out the date only on the same day in the laboratory zone", () => {
    // 04:30 UTC on 13 October is still 12 October in Chicago.
    const today = new Date("2026-10-13T04:30:00Z");
    expect(formatTimestamp("2026-10-12T14:03:05Z", "America/Chicago", { today })).toBe("09:03 CDT");
    expect(formatTimestamp("2026-10-12T14:03:05Z", null, { today })).toBe("2026-10-12 14:03 UTC");
  });
});

describe("formatBirthDate", () => {
  // Noon UTC, so the calendar date is the same in UTC and Chicago.
  const now = new Date("2026-10-13T12:00:00Z");

  it("names the month and gives the age in years from 2 years", () => {
    expect(formatBirthDate("1961-11-09", now, "America/Chicago")).toBe("Nov 9, 1961 (64 y)");
    expect(formatBirthDate("1961-10-13", now, "America/Chicago")).toBe("Oct 13, 1961 (65 y)");
    expect(formatBirthDate("2024-10-13", now, null)).toBe("Oct 13, 2024 (2 y)");
  });

  it("gives the age in months under 2 years", () => {
    expect(formatBirthDate("2024-10-14", now, null)).toBe("Oct 14, 2024 (23 mo)");
    expect(formatBirthDate("2026-02-13", now, null)).toBe("Feb 13, 2026 (8 mo)");
    expect(formatBirthDate("2026-09-13", now, null)).toBe("Sep 13, 2026 (1 mo)");
  });

  it("gives the age in days under 1 month, for neonatal specimens", () => {
    expect(formatBirthDate("2026-10-01", now, null)).toBe("Oct 1, 2026 (12 d)");
    expect(formatBirthDate("2026-09-14", now, null)).toBe("Sep 14, 2026 (29 d)");
    expect(formatBirthDate("2026-10-13", now, null)).toBe("Oct 13, 2026 (0 d)");
  });

  it("counts to today in the laboratory zone, without shifting the birth date", () => {
    // 03:00 UTC on 13 October is still 12 October in Chicago: the birthday is tomorrow there.
    const early = new Date("2026-10-13T03:00:00Z");
    expect(formatBirthDate("2025-10-13", early, "America/Chicago")).toBe("Oct 13, 2025 (11 mo)");
    expect(formatBirthDate("2025-10-13", early, null)).toBe("Oct 13, 2025 (12 mo)");
    expect(formatBirthDate("2025-10-13", early, "Not/AZone")).toBe("Oct 13, 2025 (12 mo)");
  });

  it("shows a birth date after today without an age", () => {
    expect(formatBirthDate("2026-10-14", now, null)).toBe("Oct 14, 2026");
  });

  it("shows a value that is not a calendar date as received", () => {
    expect(formatBirthDate("1961-02-30", now, null)).toBe("1961-02-30");
    expect(formatBirthDate("11/09/1961", now, null)).toBe("11/09/1961");
    expect(formatBirthDate("", now, null)).toBe("");
  });
});
