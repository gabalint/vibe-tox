import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, expect, it, vi } from "vitest";
import type { SetupStatus } from "@/generated/SetupStatus";
import { createQueryClient, useCommandQuery } from "@/lib/commands";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { DemoSetupProgress } from "./DemoSetupProgress";

function SetupHarness() {
  const setup = useCommandQuery<SetupStatus>("setup_status");
  if (setup.isPending) return <p>Checking setup</p>;
  return setup.data?.demoSetupPending ? <DemoSetupProgress /> : <p>Demo workspace</p>;
}

describe("first-run demo preparation (REQ-OPS-016)", () => {
  it("explains loading while the engine prepares the data", async () => {
    const complete = vi.fn(() => new Promise<null>(() => {}));
    mockCommands({ complete_demo_setup: complete });
    const { container } = renderWithProviders(
      <main>
        <DemoSetupProgress />
      </main>,
    );
    expect(screen.getByRole("heading", { name: "Preparing your demo laboratory" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Loading the catalog and sample data");
    expect(screen.getByText(/Catalog entries start as drafts/)).toBeTruthy();
    await waitFor(() => expect(complete).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("button")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("refreshes setup and every affected cache before opening the workspace", async () => {
    let pending = true;
    const complete = vi.fn(() => {
      pending = false;
      return null;
    });
    const status = vi.fn(() => ({ needsAdministrator: false, demoSetupPending: pending }));
    mockCommands({ complete_demo_setup: complete, setup_status: status });
    const cache = createQueryClient();
    const affected = [
      "standard_catalog_status",
      "standard_catalog_tree",
      "demo_status",
      "list_analytes",
      "list_clients",
      "list_practitioners",
      "list_delivery_destinations",
      "get_laboratory_profile",
    ];
    for (const command of affected) cache.setQueryData([command], []);
    render(
      <QueryClientProvider client={cache}>
        <SetupHarness />
      </QueryClientProvider>,
    );
    await screen.findByText("Demo workspace");
    expect(complete).toHaveBeenCalledTimes(1);
    expect(status).toHaveBeenCalledTimes(2);
    for (const command of affected) {
      expect(cache.getQueryState([command])?.isInvalidated).toBe(true);
    }
    expect(screen.queryByText("Preparing your demo laboratory")).toBeNull();
  });

  it("does not replay in StrictMode or retry errors until the person asks", async () => {
    let pending = true;
    const complete = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("conflict", "The demo could not be loaded.", { correlationId: "corr-demo" }),
      )
      .mockImplementation(() => {
        pending = false;
        return null;
      });
    mockCommands({
      setup_status: () => ({ needsAdministrator: false, demoSetupPending: pending }),
      complete_demo_setup: complete,
    });
    const { user, container } = renderWithProviders(
      <StrictMode>
        <main>
          <SetupHarness />
        </main>
      </StrictMode>,
    );
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The demo could not be loaded.");
    expect(alert.textContent).toContain("Support reference: corr-demo");
    await expectNoA11yViolations(container, { wholePage: true });
    expect(complete).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Demo workspace")).toBeNull();
    await user.tab();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Try again" }));
    await user.keyboard("{Enter}");
    await screen.findByText("Demo workspace");
    expect(complete).toHaveBeenCalledTimes(2);
  });

  it("lets someone sign out to recover from the wrong setup account", async () => {
    const signOut = vi.fn(() => null);
    const complete = vi.fn(() =>
      Promise.reject(
        engineError("conflict", "Sign in with the account created during demo setup."),
      ),
    );
    mockCommands({ complete_demo_setup: complete, sign_out: signOut });
    const { user } = renderWithProviders(<DemoSetupProgress />);
    await screen.findByText("Sign in with the account created during demo setup.");
    await user.click(screen.getByRole("button", { name: "Sign out" }));
    await waitFor(() => expect(signOut).toHaveBeenCalledTimes(1));
    expect(complete).toHaveBeenCalledTimes(1);
  });

  it("shows sign-out failures with a support reference and keeps recovery available", async () => {
    mockCommands({
      complete_demo_setup: () => Promise.reject(engineError("conflict", "Use the setup account.")),
      sign_out: () =>
        Promise.reject(
          engineError("internal", "The session could not be ended.", {
            correlationId: "corr-signout",
          }),
        ),
    });
    const { user } = renderWithProviders(<DemoSetupProgress />);
    await user.click(await screen.findByRole("button", { name: "Sign out" }));
    await screen.findByText("Sign-out failed");
    expect(screen.getByText("corr-signout")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Sign out" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});
