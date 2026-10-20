import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { DemoBanner } from "./DemoBanner";

describe("DemoBanner (proposed REQ-OPS-016)", () => {
  it("marks the demo laboratory as not for patient testing", async () => {
    mockCommands({});
    const { container } = renderWithProviders(<DemoBanner />);
    const banner = screen.getByRole("region", { name: "Demo laboratory" });
    expect(banner.textContent).toContain("DEMO LABORATORY — not for patient testing");
    await expectNoA11yViolations(container);
  });

  it("leaves the demo laboratory after confirmation", async () => {
    const leave = vi.fn(() => null);
    mockCommands({ leave_demo_laboratory: leave });
    const { user } = renderWithProviders(<DemoBanner />);
    await user.click(screen.getByRole("button", { name: "Leave the demo laboratory" }));
    const dialog = screen.getByRole("alertdialog", { name: "Leave the demo laboratory?" });
    expect(leave).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "Leave the demo laboratory" }));
    expect(await within(dialog).findByRole("status")).toHaveProperty("textContent", "Restarting…");
    expect(leave).toHaveBeenCalledTimes(1);
  });

  it("resets only after the destructive confirmation, and Cancel does nothing", async () => {
    const reset = vi.fn(() => null);
    mockCommands({ reset_demo_laboratory: reset });
    const { user } = renderWithProviders(<DemoBanner />);
    await user.click(screen.getByRole("button", { name: "Reset the demo laboratory" }));
    let dialog = screen.getByRole("alertdialog", { name: "Reset the demo laboratory?" });
    expect(dialog.textContent).toContain("The laboratory's own data is not touched.");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(reset).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Reset the demo laboratory" }));
    dialog = screen.getByRole("alertdialog", { name: "Reset the demo laboratory?" });
    await user.click(within(dialog).getByRole("button", { name: "Delete the demo data" }));
    await within(dialog).findByText("Restarting…");
    expect(reset).toHaveBeenCalledTimes(1);
  });

  it("keeps a refusal in the dialog with its support reference", async () => {
    mockCommands({
      leave_demo_laboratory: () =>
        Promise.reject(engineError("conflict", "Wait until these finish, then try again.")),
    });
    const { user } = renderWithProviders(<DemoBanner />);
    await user.click(screen.getByRole("button", { name: "Leave the demo laboratory" }));
    const dialog = screen.getByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Leave the demo laboratory" }));
    expect(
      await within(dialog).findByText("Wait until these finish, then try again."),
    ).toBeTruthy();
    expect(within(dialog).getByText("corr-test-0001")).toBeTruthy();
    expect(within(dialog).queryByText("Restarting…")).toBeNull();
  });
});
