import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import type { ActionItem } from "./ActionMenu";
import { RecordActions } from "./RecordActions";

const make = (count: number, onAction: (id: string) => void = vi.fn()): ActionItem[] =>
  Array.from({ length: count }, (_, index) => ({
    id: `action-${index}`,
    label: `Action ${index}`,
    onAction: () => onAction(`action-${index}`),
  }));

describe("RecordActions", () => {
  it("renders nothing without items", () => {
    const { container } = renderWithProviders(<RecordActions items={[]} />);
    expect(container.textContent).toBe("");
  });

  it("shows one item as the primary button", () => {
    renderWithProviders(<RecordActions items={make(1)} />);
    expect(screen.getByRole("button", { name: "Action 0" }).dataset.variant).toBe("primary");
    expect(screen.getByRole("button", { name: "Action 0" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
  });

  it("shows three items as buttons without More", () => {
    renderWithProviders(<RecordActions items={make(3)} />);
    expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
      "Action 0",
      "Action 1",
      "Action 2",
    ]);
  });

  it("puts items past the third under More", async () => {
    const onAction = vi.fn();
    const { user } = renderWithProviders(<RecordActions items={make(5, onAction)} />);
    expect(screen.queryByRole("button", { name: "Action 3" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More" }));
    expect(await screen.findByRole("menuitem", { name: "Action 3" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Action 4" })).toBeTruthy();
    await user.click(screen.getByRole("menuitem", { name: "Action 4" }));
    expect(onAction).toHaveBeenCalledWith("action-4");
  });

  it("runs a button's action", async () => {
    const onAction = vi.fn();
    const { user } = renderWithProviders(<RecordActions items={make(2, onAction)} />);
    await user.click(screen.getByRole("button", { name: "Action 1" }));
    expect(onAction).toHaveBeenCalledWith("action-1");
  });

  it("keeps a destructive item as a danger button, even first", async () => {
    const items: ActionItem[] = [
      { id: "retire", label: "Retire", onAction: vi.fn(), variant: "danger" },
      ...make(1),
    ];
    const { container } = renderWithProviders(<RecordActions items={items} />);
    const retire = screen.getByRole("button", { name: "Retire" });
    const other = screen.getByRole("button", { name: "Action 0" });
    expect(retire.dataset.variant).toBe("danger");
    expect(other.dataset.variant).toBe("secondary");
    await expectNoA11yViolations(container);
  });

  it("has no accessibility violations with a More menu", async () => {
    const { container } = renderWithProviders(<RecordActions items={make(5)} />);
    await expectNoA11yViolations(container);
  });
});
