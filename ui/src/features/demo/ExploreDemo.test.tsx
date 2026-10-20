import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ExploreDemo } from "./ExploreDemo";

function renderEntry(installation: string, open = vi.fn(() => null)) {
  mockCommands({
    startup_status: () => ({ state: "running", code: null, dataFolder: null, installation }),
    open_demo_laboratory: open,
  });
  return { ...renderWithProviders(<ExploreDemo />), open };
}

describe("ExploreDemo (proposed REQ-OPS-016)", () => {
  it("opens the demo laboratory from a laboratory installation after confirmation", async () => {
    const { user, open, container } = renderEntry("laboratory");
    const explore = await screen.findByRole("button", { name: "Explore the demo laboratory" });
    await expectNoA11yViolations(container);
    await user.click(explore);
    const dialog = screen.getByRole("alertdialog", { name: "Open the demo laboratory?" });
    expect(open).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Open the demo laboratory" }));
    await within(dialog).findByText("Restarting…");
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("is not offered inside the demo laboratory", async () => {
    const status = vi.fn(() => ({
      state: "running",
      code: null,
      dataFolder: null,
      installation: "demo",
    }));
    mockCommands({ startup_status: status });
    const { container } = renderWithProviders(<ExploreDemo />);
    await vi.waitFor(() => expect(status).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container.textContent).toBe("");
  });
});
