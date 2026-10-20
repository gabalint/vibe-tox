import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RestoreAvailability } from "@/generated/RestoreAvailability";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthLayout } from "./AuthLayout";
import { RestoreFlowContext } from "./restoreSlot";
import { SetupScreen } from "./SetupScreen";

const AVAILABLE: RestoreAvailability = { available: true, pending: null, lastFailureCode: null };
const PENDING = { setId: "20261013T140000Z-0000b3aa", setCreatedAt: "2026-10-13T14:00:00Z" };

function FakeRestoreFlow({ onExit }: { onExit: () => void }) {
  return (
    <>
      <h1>Synthetic restore flow</h1>
      <button type="button" onClick={onExit}>
        Leave the restore
      </button>
    </>
  );
}

function renderSetup(availability: () => unknown, { withFlow = true } = {}) {
  mockCommands({
    startup_status: () => ({ installation: "laboratory" }),
    get_restore_availability: availability,
  });
  const screenUnderTest = (
    <AuthLayout>
      <SetupScreen onCreated={vi.fn()} />
    </AuthLayout>
  );
  return renderWithProviders(
    withFlow ? (
      <RestoreFlowContext.Provider value={FakeRestoreFlow}>
        {screenUnderTest}
      </RestoreFlowContext.Provider>
    ) : (
      screenUnderTest
    ),
  );
}

async function setupHeading() {
  return screen.findByRole("heading", { level: 1, name: "Set up this laboratory" });
}

describe("SetupScreen restore entry (M4 design brief §3.7)", () => {
  it("opens the provided restore flow and returns to setup from it", async () => {
    const { user, container } = renderSetup(() => AVAILABLE);
    await user.click(await screen.findByRole("button", { name: "Restore from a backup" }));
    expect(screen.getByRole("heading", { name: "Synthetic restore flow" })).toBeTruthy();
    expect(screen.queryByRole("form", { name: "Create the first administrator" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "Leave the restore" }));
    expect(await setupHeading()).toBeTruthy();
    expect(screen.getByRole("button", { name: "Restore from a backup" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it.each([
    ["the engine offers no restore", () => ({ ...AVAILABLE, available: false }), true],
    ["no restore flow is provided", () => AVAILABLE, false],
    ["the availability cannot be read", () => Promise.reject(engineError("internal", "No.")), true],
  ])("offers no restore when %s", async (_case, availability, withFlow) => {
    const query = vi.fn(availability);
    renderSetup(query, { withFlow });
    expect(await setupHeading()).toBeTruthy();
    await vi.waitFor(() => expect(query).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Restore from a backup" })).toBeNull();
    expect(screen.getByRole("button", { name: "Create administrator" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("says a confirmed restore finishes when the app opens again", async () => {
    const { container } = renderSetup(() => ({
      available: false,
      pending: PENDING,
      lastFailureCode: null,
    }));
    expect(await screen.findByText("A restore is ready to finish")).toBeTruthy();
    expect(screen.getByText("2026-10-13 14:00 UTC")).toBeTruthy();
    expect(screen.getByText(/Close the app and open it again to finish\./)).toBeTruthy();
    const notice = screen.getByText("A restore is ready to finish");
    const create = screen.getByRole("button", { name: "Create administrator" });
    expect(notice.compareDocumentPosition(create) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("says the last restore could not be applied and the current data was kept", async () => {
    const { container } = renderSetup(() => ({
      available: false,
      pending: PENDING,
      lastFailureCode: "users_exist",
    }));
    expect(await screen.findByText("The last restore could not be applied")).toBeTruthy();
    expect(
      screen.getByText(
        /User accounts were created on this computer before the restore was applied\. The current data was kept\./,
      ),
    ).toBeTruthy();
    expect(screen.queryByText("A restore is ready to finish")).toBeNull();
    expect(screen.queryByRole("button", { name: "Restore from a backup" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows an unknown failure code as the engine sent it", async () => {
    renderSetup(() => ({ available: false, pending: PENDING, lastFailureCode: "new_code" }));
    expect(await screen.findByText(/Code: new_code The current data was kept\./)).toBeTruthy();
  });
});
