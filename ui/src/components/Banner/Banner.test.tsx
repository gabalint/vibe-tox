import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StatusBadge } from "@/components/StatusBadge";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { Banner } from "./Banner";

describe("Banner", () => {
  it("is a labelled region of facts with one emphasized and a status", async () => {
    const { container } = renderWithProviders(
      <Banner
        label="Synthetic batch"
        items={[
          { label: "Batch", value: "SYN-000042", emphasis: true },
          { label: "Positions", value: "12" },
        ]}
        status={<StatusBadge tone="hold">On hold</StatusBadge>}
      />,
    );
    const region = screen.getByRole("region", { name: "Synthetic batch" });
    const terms = within(region)
      .getAllByRole("term")
      .map((term) => term.textContent);
    const values = within(region)
      .getAllByRole("definition")
      .map((value) => value.textContent);
    expect(terms).toEqual(["Batch", "Positions"]);
    expect(values).toEqual(["SYN-000042", "12"]);
    expect(screen.getByText("SYN-000042").parentElement?.hasAttribute("data-emphasis")).toBe(true);
    expect(within(region).getByText("On hold")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("shows no status area without a status", () => {
    renderWithProviders(
      <Banner label="Synthetic batch" items={[{ label: "Batch", value: "B" }]} />,
    );
    expect(screen.getByRole("region", { name: "Synthetic batch" }).textContent).toBe("BatchB");
  });

  describe("in a scrolling screen", () => {
    afterEach(() => vi.unstubAllGlobals());

    it("keeps focused elements from scrolling under it, and lets go when it leaves", () => {
      vi.stubGlobal(
        "ResizeObserver",
        class {
          constructor(private readonly callback: () => void) {}
          observe() {
            this.callback();
          }
          disconnect() {}
        },
      );
      vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue(
        DOMRect.fromRect({ height: 120 }),
      );
      const { unmount } = renderWithProviders(
        <main style={{ overflow: "auto" }}>
          <div>
            <Banner label="Synthetic batch" items={[{ label: "Batch", value: "B" }]} />
          </div>
        </main>,
      );
      const main = screen.getByRole("main");
      expect(main.style.scrollPaddingTop).toBe("calc(120px + var(--space-2))");
      unmount();
      expect(main.style.scrollPaddingTop).toBe("");
    });
  });
});
