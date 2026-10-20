import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { typeInto } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { healthCheck, hostHealth, sessionWith } from "../testData";
import { HealthPage } from "./HealthPage";

const UPS = healthCheck({
  check: "ups",
  label: "UPS",
  state: "unknown",
  detailCode: "not_readable",
  summary: "The app cannot read the UPS.",
  value: null,
  threshold: null,
  ownerCapability: "manage_configuration",
  nextAction: "Check the UPS and attest it.",
  attestable: true,
});

const HEALTH = hostHealth({
  overall: "warning",
  checks: [healthCheck(), UPS],
  keepAwake: { state: "held", since: "2026-10-13T13:00:00Z", code: null },
});

function renderPage(
  capabilities: string[],
  commands: Record<string, (args: unknown) => unknown> = {},
  path = "/operations/health",
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_host_health: () => HEALTH,
    ...commands,
  });
  return renderRoutes([{ path: "/operations/health", element: <HealthPage /> }], path);
}

describe("HealthPage (REQ-OPS-008, REQ-OPS-013)", () => {
  beforeEach(mockGridLayout);

  it("shows the checks and this computer, and attests a check the app cannot read", async () => {
    const attest = vi.fn(() => ({ ...UPS, state: "attested" }));
    const { user } = renderPage(["manage_configuration"], {
      attest_health_check: attest,
    });
    expect(await screen.findByRole("gridcell", { name: "UPS" })).toBeTruthy();
    expect(screen.getByText("Overall").parentElement?.textContent).toContain("Warning");
    // The limit shows beneath the value; the detail code is a machine code, kept in the history.
    expect(screen.getByText("Warning at 2 h, alert at 4 h")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Check the UPS and attest it." })).toBeTruthy();
    expect(screen.queryByText("not_readable")).toBeNull();
    expect(screen.getByText("Keeping the computer awake")).toBeTruthy();
    expect(screen.getByText("WebKitGTK 2.44")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Health history" })).toBeTruthy();
    expect(screen.queryByText(/Select a check/)).toBeNull();
    await openRow(user, "UPS");
    const panel = screen.getByRole("region", { name: "UPS" });
    await user.click(within(panel).getByRole("button", { name: "Attest" }));
    const dialog = await screen.findByRole("dialog", { name: "Attest check" });
    await user.click(within(dialog).getByRole("button", { name: "Attest" }));
    expect(await within(dialog).findByText("Enter note.")).toBeTruthy();
    expect(within(dialog).getByText(/^UPS: you confirm/)).toBeTruthy();
    await typeInto(user, /^Note/, "UPS self-test passed");
    await user.click(within(dialog).getByRole("button", { name: "Attest" }));
    expect(await screen.findByText("Attestation recorded")).toBeTruthy();
    expect(attest).toHaveBeenCalledWith({ input: { check: "ups", note: "UPS self-test passed" } });
  });

  it("puts this computer's facts above the checks, which take the remaining height", async () => {
    renderPage([]);
    const grid = await screen.findByRole("grid", { name: "Health checks" });
    const facts = screen.getByRole("heading", { level: 2, name: "This computer" });
    expect(facts.compareDocumentPosition(grid) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("opens a check by click and by Enter, and Escape closes the panel with focus on the row", async () => {
    const { user, router, container } = renderPage(["manage_configuration"]);
    await openRow(user, "Backup age");
    expect(router.state.location.search).toBe("?open=backup_age");
    const panel = screen.getByRole("region", { name: "Backup age" });
    // An ordinary check has nothing to attest: no button, and no row menu.
    expect(within(panel).queryByRole("button", { name: "Attest" })).toBeNull();
    expect(
      within(panel).getByText("The newest verified copy covers every audit event."),
    ).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    // The most urgent check is listed first, so Previous reaches the UPS.
    await user.click(within(panel).getByRole("button", { name: "Previous" }));
    expect(screen.getByRole("region", { name: "UPS" })).toBeTruthy();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "UPS" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "UPS" })),
    );
    await openRowWithEnter(user, "Backup age");
    expect(screen.getByRole("region", { name: "Backup age" })).toBeTruthy();
  });

  it("opens the check named in the address, in the panel with its attest action", async () => {
    renderPage(["manage_configuration"], {}, "/operations/health?open=ups");
    const panel = await screen.findByRole("region", { name: "UPS" });
    // The capability arrives with the session, after the panel is first shown.
    expect(await within(panel).findByRole("button", { name: "Attest" })).toBeTruthy();
    expect(within(panel).getByText("Check the UPS and attest it.")).toBeTruthy();
  });

  it("says when the address names no check", async () => {
    renderPage([], {}, "/operations/health?open=nothing");
    expect(await screen.findByRole("region", { name: "Record not found" })).toBeTruthy();
  });

  it("offers attestation in the row's menu only for an attestable check, to people who may", async () => {
    const { user, unmount } = renderPage(["manage_configuration"]);
    await screen.findByRole("gridcell", { name: "UPS" });
    expect(screen.queryByRole("button", { name: "Actions for Backup age" })).toBeNull();
    const menu = await openRowActions(user, "UPS");
    await user.click(within(menu).getByRole("menuitem", { name: "Attest" }));
    expect(await screen.findByRole("dialog", { name: "Attest check" })).toBeTruthy();
    unmount();

    renderPage([], {}, "/operations/health?open=ups");
    await screen.findByRole("region", { name: "UPS" });
    expect(screen.queryByRole("button", { name: "Attest" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for UPS" })).toBeNull();
  });

  it("lists the most urgent check first and shows an attested check as finished", async () => {
    renderPage([], {
      get_host_health: () =>
        hostHealth({
          checks: [
            healthCheck(),
            healthCheck({ check: "ups", label: "UPS", state: "attested" }),
            healthCheck({ check: "disk_free", label: "Free disk space", state: "alert" }),
          ],
        }),
    });
    await screen.findByRole("gridcell", { name: "UPS" });
    const labels = screen
      .getAllByRole("row")
      .slice(1)
      .map((row) => within(row).getAllByRole("gridcell")[0]?.textContent);
    expect(labels).toEqual(["Free disk space", "UPS", "Backup age"]);
  });

  it("checks again, and says so when asked again within a minute", async () => {
    const refresh = vi
      .fn()
      .mockReturnValueOnce(HEALTH)
      .mockImplementationOnce(() =>
        Promise.reject(
          engineError(
            "conflict",
            "Health was refreshed less than a minute ago. Try again in a minute.",
            {
              gateReasons: [
                { code: "refresh_rate_limited", message: "Refresh is allowed once a minute." },
              ],
            },
          ),
        ),
      );
    const { user, container } = renderPage([], { refresh_host_health: refresh });
    await user.click(await screen.findByRole("button", { name: "Check now" }));
    expect(await screen.findByText("Checks run again")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Check now" }));
    expect(await screen.findByText("Checked less than a minute ago")).toBeTruthy();
    expect(screen.getByText(/Try again in a minute/)).toBeTruthy();
    expect(screen.queryByText("Not checked again")).toBeNull();
    expect(screen.queryByRole("button", { name: "Attest" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows any other refusal of a refresh as an error", async () => {
    const { user } = renderPage([], {
      refresh_host_health: () =>
        Promise.reject(engineError("internal", "The checks could not run.")),
    });
    await user.click(await screen.findByRole("button", { name: "Check now" }));
    expect(await screen.findByText("Not checked again")).toBeTruthy();
    expect(screen.getByText("The checks could not run.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("says when no check is reported", async () => {
    const { container } = renderPage([], { get_host_health: () => hostHealth({ checks: [] }) });
    expect(await screen.findByText("No checks reported")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's error", async () => {
    renderPage([], {
      get_host_health: () => Promise.reject(engineError("internal", "Health is unavailable.")),
    });
    expect(await screen.findByText("Health is unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
