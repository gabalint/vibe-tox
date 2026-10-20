import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The path is relative to the repository root, where the npm scripts run Vitest.
// Docs: docs/governance/ui-standards.md#color. Contrast follows WCAG 2.x relative luminance.
const tokensCss = readFileSync("ui/src/styles/tokens.css", "utf8");
const declarations = new Map<string, string>();
for (const match of tokensCss.matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
  const [, name, value] = match;
  if (name !== undefined && value !== undefined) declarations.set(name, value.trim());
}

function resolve(name: string): string {
  const value = declarations.get(name);
  if (value === undefined) throw new Error(`Missing token ${name}`);
  const reference = /^var\((--[\w-]+)\)$/.exec(value);
  return reference?.[1] === undefined ? value : resolve(reference[1]);
}

function channel(hex: string, offset: number): number {
  const c = Number.parseInt(hex.slice(offset, offset + 2), 16) / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(token: string): number {
  const hex = resolve(token);
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error(`${token} is not a 6-digit hex color`);
  return 0.2126 * channel(hex, 1) + 0.7152 * channel(hex, 3) + 0.0722 * channel(hex, 5);
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  if (hi === undefined || lo === undefined) throw new Error("unreachable");
  return (hi + 0.05) / (lo + 0.05);
}

const white = "--color-surface";
const page = "--color-surface-sunken";
const rowTint = "--color-accent-subtle";
const tones = ["info", "warning", "success", "danger", "hold", "neutral"] as const;

// [foreground, background, minimum ratio, documented ratio or null]
const pairs: [string, string, number, number | null][] = [
  ["--color-text", white, 4.5, 16.6],
  ["--color-text-muted", page, 4.5, 6.3],
  ["--color-text-muted", rowTint, 4.5, 5.8],
  ["--color-border-control", white, 3, 4.7],
  ["--color-accent", white, 4.5, 6.7],
  ["--color-text-inverse", "--color-accent", 4.5, 6.7],
  ["--color-text-inverse", "--color-accent-hover", 4.5, 8.7],
  ["--color-text", rowTint, 4.5, 14.3],
  ["--color-focus", white, 3, 6.7],
  ["--color-focus", rowTint, 3, 5.8],
  ["--tone-danger-text", white, 4.5, null],
  ["--tone-danger-text", "--color-surface-hover", 4.5, null],
  ["--color-text-inverse", "--tone-hold-solid", 4.5, null],
  ["--color-text-inverse", "--tone-danger-solid", 4.5, null],
  ...tones.map((t): [string, string, number, number | null] => [
    `--tone-${t}-text`,
    `--tone-${t}-bg`,
    6.6,
    null,
  ]),
  ["--color-text-muted", "--color-surface-hover", 4.5, null],
  ["--tone-neutral-text", white, 4.5, null],
  ["--tone-danger-text", "--tone-danger-bg", 4.5, null],
];

describe("design tokens", () => {
  it.each(pairs)("%s on %s is at least %d:1", (fg, bg, minimum, documented) => {
    const ratio = contrast(fg, bg);
    expect(ratio).toBeGreaterThanOrEqual(minimum);
    if (documented !== null) expect(Math.abs(ratio - documented)).toBeLessThanOrEqual(0.1);
  });

  it.each(["info", "warning", "success"] as const)(
    "the %s border is at least 3:1 on white",
    (t) => {
      expect(contrast(`--tone-${t}-border`, white)).toBeGreaterThanOrEqual(3);
    },
  );

  it.each(["danger", "hold"] as const)("the %s solid fill is at least 3:1 on white", (t) => {
    expect(contrast(`--tone-${t}-solid`, white)).toBeGreaterThanOrEqual(3);
    expect(resolve(`--tone-${t}-border`)).toBe(resolve(`--tone-${t}-solid`));
  });

  it("uses the accent for the focus ring", () => {
    expect(resolve("--color-focus")).toBe(resolve("--color-accent"));
  });
});
