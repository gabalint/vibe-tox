import { describe, expect, it } from "vitest";

// REQ-RPT-011: delivery status always names the channel and the evidence level, and only the
// engine builds it. No UI source may claim delivery with a bare word. The word is assembled
// here so this file does not match itself.
const BARE_WORD = ["Deliver", "ed"].join("");

const sources = import.meta.glob<string>("/src/**/*.{ts,tsx}", {
  query: "?raw",
  import: "default",
  eager: true,
});

describe("delivery wording (REQ-RPT-011)", () => {
  it("scans the UI sources", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(100);
  });

  it("never writes a bare delivery claim anywhere under ui/src", () => {
    const offenders = Object.entries(sources)
      .filter(([, text]) => text.includes(BARE_WORD))
      .map(([path]) => path);
    expect(offenders).toEqual([]);
  });
});
