import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { DemoEntryContext } from "./demoSlot";
import { SetupScreen } from "./SetupScreen";

function FakeDemoEntry() {
  return <p>Synthetic demo entry</p>;
}

describe("SetupScreen demo entry (proposed REQ-OPS-016)", () => {
  it("offers the demo entry the app frame provides, beside creating the administrator", async () => {
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
    });
    renderWithProviders(
      <DemoEntryContext.Provider value={FakeDemoEntry}>
        <SetupScreen onCreated={vi.fn()} />
      </DemoEntryContext.Provider>,
    );
    expect(await screen.findByText("Synthetic demo entry")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Create administrator" })).toBeTruthy();
  });

  it("offers none without a provider", async () => {
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
    });
    renderWithProviders(<SetupScreen onCreated={vi.fn()} />);
    expect(await screen.findByRole("button", { name: "Create administrator" })).toBeTruthy();
    expect(screen.queryByText("Synthetic demo entry")).toBeNull();
  });
});
