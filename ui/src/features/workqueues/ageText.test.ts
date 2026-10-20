import { describe, expect, it } from "vitest";
import { ageText } from "./ageText";

const NOW = Date.parse("2026-10-13T12:00:00Z");

describe("ageText (REQ-OPS-004)", () => {
  it.each([
    ["2026-10-13T12:00:00Z", "Under 1 min"],
    ["2026-10-13T11:59:30Z", "Under 1 min"],
    ["2026-10-13T12:05:00Z", "Under 1 min"],
    ["2026-10-13T11:59:00Z", "1 min"],
    ["2026-10-13T11:01:00Z", "59 min"],
    ["2026-10-13T11:00:00Z", "1 h 0 min"],
    ["2026-10-13T08:40:00Z", "3 h 20 min"],
    ["2026-10-12T12:00:01Z", "23 h 59 min"],
    ["2026-10-12T12:00:00Z", "1 d 0 h"],
    ["2026-10-11T08:00:00Z", "2 d 4 h"],
    ["2026-10-13T07:00:00-05:00", "Under 1 min"],
    ["not a time", "Unknown"],
  ])("from %s reads %s", (since, expected) => {
    expect(ageText(since, NOW)).toBe(expected);
  });
});
