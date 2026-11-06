import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Tone } from "@/components/tones";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { StatusBadge } from "./StatusBadge";

const examples: [Tone, string][] = [
  ["neutral", "Approved"],
  ["info", "Queued"],
  ["warning", "Draft awaiting approval"],
  ["hold", "Identity hold"],
  ["danger", "Label print failed"],
  ["success", "Restore test passed"],
];

describe("StatusBadge", () => {
  it.each(examples)("states a %s status in text with an icon", async (tone, label) => {
    const { container } = renderWithProviders(<StatusBadge tone={tone}>{label}</StatusBadge>);
    const badge = screen.getByText(label);
    expect(badge.getAttribute("data-tone")).toBe(tone);
    expect(badge.querySelector("svg[aria-hidden='true']")).not.toBeNull();
    await expectNoA11yViolations(container);
  });

  it("fills its cell only when asked, so a column of statuses lines up", () => {
    renderWithProviders(
      <>
        <StatusBadge tone="danger" fill>
          Rejected
        </StatusBadge>
        <StatusBadge tone="success">Order created</StatusBadge>
      </>,
    );
    expect(screen.getByText("Rejected").hasAttribute("data-fill")).toBe(true);
    expect(screen.getByText("Order created").hasAttribute("data-fill")).toBe(false);
  });
});
