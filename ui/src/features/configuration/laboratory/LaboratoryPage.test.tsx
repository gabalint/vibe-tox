import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { LaboratoryProfileView } from "@/generated/LaboratoryProfileView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { MANAGER, sessionWith } from "../testConfiguration";
import { LaboratoryPage } from "./LaboratoryPage";

const PROFILE: LaboratoryProfileView = {
  profileId: "00000000-0000-7000-8000-000000000d71",
  version: 1,
  name: "Synthetic Toxicology Laboratory",
  addressLines: ["100 Test Street", "Suite 1"],
  cliaId: "00D0000000",
  director: "Test Director",
  phone: "312-555-0100",
  timeZone: "America/Chicago",
  billingNpi: null,
  taxId: null,
  reason: null,
  setBy: "00000000-0000-7000-8000-000000000e09",
  setByDisplay: "Test Profile Owner",
  setAt: "2026-10-12T14:03:00Z",
};

function renderPage(
  getProfile: () => unknown,
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    get_laboratory_profile: getProfile,
    ...commands,
  });
  return renderWithProviders(<LaboratoryPage />);
}

async function fillFirstProfile(user: ReturnType<typeof renderPage>["user"]) {
  await user.keyboard("Synthetic Toxicology Laboratory");
  await typeInto(user, /^Address line 1/, "100 Test Street");
  await typeInto(user, /^Address line 3/, "Floor 2");
  await typeInto(user, /^CLIA number/, "00D0000000");
  await typeInto(user, /^Laboratory director/, "Test Director");
  await typeInto(user, /^Phone/, "312-555-0100");
  await typeInto(user, /^Time zone/, "America/Chicago");
}

describe("LaboratoryPage (REQ-RPT-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("explains a missing profile and sets the first one", async () => {
    const set = vi.fn(() => PROFILE);
    const { user, container } = renderPage(() => null, { set_laboratory_profile: set });
    expect(await screen.findByText("No laboratory profile")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Set laboratory profile" }));
    const dialog = await screen.findByRole("dialog", { name: "Set laboratory profile" });
    expect(within(dialog).queryByRole("textbox", { name: /^Reason/ })).toBeNull();
    await fillFirstProfile(user);
    await user.click(within(dialog).getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("Laboratory profile saved")).toBeTruthy();
    expect(set).toHaveBeenCalledWith({
      input: {
        expectedVersion: null,
        name: "Synthetic Toxicology Laboratory",
        addressLines: ["100 Test Street", "Floor 2"],
        cliaId: "00D0000000",
        director: "Test Director",
        phone: "312-555-0100",
        timeZone: "America/Chicago",
        billingNpi: null,
        taxId: null,
      },
    });
  });

  it("shows the current profile and changes it with a reason", async () => {
    const set = vi.fn(() => ({ ...PROFILE, version: 2 }));
    const { user, container } = renderPage(() => PROFILE, { set_laboratory_profile: set });
    expect(await screen.findByText("Synthetic Toxicology Laboratory")).toBeTruthy();
    expect(screen.getByText("100 Test Street, Suite 1")).toBeTruthy();
    expect(screen.getAllByText("Not set")).toHaveLength(2);
    expect(screen.getByText("2026-10-12 14:03 UTC")).toBeTruthy();
    // Who set it shows by the engine's display name, never the internal user ID.
    expect(screen.getByText(/by Test Profile Owner/)).toBeTruthy();
    expect(screen.queryByText(/000000000e09/)).toBeNull();
    expect(screen.getByText("None (first version)")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Change profile" }));
    const dialog = await screen.findByRole("dialog", { name: "Change laboratory profile" });
    await user.click(within(dialog).getByRole("button", { name: "Save profile" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(set).not.toHaveBeenCalled();
    await typeInto(user, /^Billing NPI/, "1234567893");
    await typeInto(user, /^Reason/, "Billing NPI assigned");
    await user.click(within(dialog).getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("Laboratory profile saved")).toBeTruthy();
    expect(set).toHaveBeenCalledWith({
      input: {
        expectedVersion: 1,
        name: "Synthetic Toxicology Laboratory",
        addressLines: ["100 Test Street", "Suite 1"],
        cliaId: "00D0000000",
        director: "Test Director",
        phone: "312-555-0100",
        timeZone: "America/Chicago",
        billingNpi: "1234567893",
        taxId: null,
        reason: "Billing NPI assigned",
      },
    });
  });

  it("shows the engine's field errors on the matching fields", async () => {
    const set = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "timeZone", code: "invalid_time_zone", message: "Unknown time zone." },
            { field: "addressLines[0]", code: "too_long", message: "Shorten this line." },
          ],
        }),
      ),
    );
    const { user } = renderPage(() => null, { set_laboratory_profile: set });
    await user.click(await screen.findByRole("button", { name: "Set laboratory profile" }));
    const dialog = await screen.findByRole("dialog", { name: "Set laboratory profile" });
    await fillFirstProfile(user);
    await user.click(within(dialog).getByRole("button", { name: "Save profile" }));
    const zone = await within(dialog).findByRole("textbox", { name: /^Time zone/ });
    expect(await within(dialog).findByText("Unknown time zone.")).toBeTruthy();
    expect(zone.getAttribute("aria-invalid")).toBe("true");
    const line1 = within(dialog).getByRole("textbox", { name: /^Address line 1/ });
    expect(line1.getAttribute("aria-invalid")).toBe("true");
  });

  it("shows a stale-version conflict in the dialog", async () => {
    const set = vi.fn(() =>
      Promise.reject(
        engineError("conflict", "The laboratory profile changed since you opened it."),
      ),
    );
    const { user } = renderPage(() => PROFILE, { set_laboratory_profile: set });
    await user.click(await screen.findByRole("button", { name: "Change profile" }));
    const dialog = await screen.findByRole("dialog", { name: "Change laboratory profile" });
    await typeInto(user, /^Reason/, "Phone changed");
    await user.click(within(dialog).getByRole("button", { name: "Save profile" }));
    expect(
      await within(dialog).findByText("The laboratory profile changed since you opened it."),
    ).toBeTruthy();
  });

  it("offers no change without manage configuration", async () => {
    mockCommands({
      current_session: () => sessionWith([]),
      get_laboratory_profile: () => PROFILE,
    });
    renderWithProviders(<LaboratoryPage />);
    expect(await screen.findByText("Synthetic Toxicology Laboratory")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Change profile" })).toBeNull();
  });

  it("shows the engine's error", async () => {
    renderPage(() => Promise.reject(engineError("internal", "The profile is unavailable.")));
    expect(await screen.findByText("The profile is unavailable.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Laboratory profile" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Change profile" })).toBeNull();
  });
});
