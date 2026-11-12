import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { BrandLockup } from "./BrandLockup";

describe("BrandLockup", () => {
  it("names the product over its parent project, with a decorative mark", async () => {
    const { container } = renderWithProviders(
      <p>
        <BrandLockup />
      </p>,
    );
    expect(container.textContent).toBe("Steady State An OpenBGA project");
    expect(container.querySelector("img")?.getAttribute("alt")).toBe("");
    await expectNoA11yViolations(container);
  });
});
