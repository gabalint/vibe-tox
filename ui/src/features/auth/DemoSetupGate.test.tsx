import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";
import { DemoSetupContext } from "./demoSetupSlot";
import { TEST_SESSION } from "./testSession";

function DemoPreparation() {
  return <h1>Preparing demo fixtures</h1>;
}

function renderGate() {
  return renderWithProviders(
    <DemoSetupContext.Provider value={DemoPreparation}>
      <AuthGate>{() => <p>Laboratory workspace</p>}</AuthGate>
    </DemoSetupContext.Provider>,
  );
}

describe("AuthGate demo setup completion", () => {
  it("waits for the setup status before allowing entry", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      setup_status: () => new Promise<never>(() => {}),
    });
    renderGate();
    await screen.findByText("Checking setup…");
    expect(screen.queryByText("Laboratory workspace")).toBeNull();
    expect(screen.queryByText("Preparing demo fixtures")).toBeNull();
  });

  it("runs pending demo preparation only after sign-in", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      setup_status: () => ({ needsAdministrator: false, demoSetupPending: true }),
    });
    renderGate();
    await screen.findByRole("heading", { name: "Preparing demo fixtures" });
    expect(screen.queryByText("Laboratory workspace")).toBeNull();
  });

  it.each<{ label: string; session: SessionInfo | null }>([
    { label: "signed out", session: null },
    { label: "locked", session: { ...TEST_SESSION, locked: true } },
  ])("does not prepare data while $label", async ({ session }) => {
    const currentSession = vi.fn(() => session);
    mockCommands({
      current_session: currentSession,
      setup_status: () => ({ needsAdministrator: false, demoSetupPending: true }),
    });
    renderGate();
    await screen.findByLabelText(/^Password/);
    expect(screen.queryByText("Preparing demo fixtures")).toBeNull();
    expect(screen.queryByText("Laboratory workspace")).toBeNull();
  });

  it("opens the workspace directly when demo preparation is not pending", async () => {
    mockCommands({
      current_session: () => TEST_SESSION,
      setup_status: () => ({ needsAdministrator: false, demoSetupPending: false }),
    });
    renderGate();
    await screen.findByText("Laboratory workspace");
    expect(screen.queryByText("Preparing demo fixtures")).toBeNull();
  });

  it("keeps the workspace closed when status fails and allows an explicit retry", async () => {
    const setup = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "The setup status could not be checked."))
      .mockResolvedValue({ needsAdministrator: false, demoSetupPending: false });
    mockCommands({ current_session: () => TEST_SESSION, setup_status: setup });
    const { user } = renderGate();
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Support reference: corr-test-0001",
    );
    expect(setup).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Laboratory workspace")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByText("Laboratory workspace");
    expect(setup).toHaveBeenCalledTimes(2);
  });
});
