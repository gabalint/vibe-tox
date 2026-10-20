import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { OperationsSettings } from "@/generated/OperationsSettings";
import type { OperationsSettingsView } from "@/generated/OperationsSettingsView";
import type { SecurityPolicyView } from "@/generated/SecurityPolicyView";
import type { SeparationPolicyView } from "@/generated/SeparationPolicyView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { OperationsSettingsPage } from "./OperationsSettingsPage";

const DEFAULTS: OperationsSettings = {
  backupIntervalMinutes: 60,
  keepRecent: 6,
  keepDaily: 7,
  keepWeekly: 4,
  backupWarnMinutes: 120,
  backupAlertMinutes: 240,
  offhostWarnHours: 24,
  offlineWarnDays: 7,
  restoreTestWarnDays: 90,
  kitConfirmWarnDays: 90,
  anchorWarnMinutes: 30,
  anchorIncidentHours: 24,
  diskFreeWarnGib: 10,
  osSupportWarnDays: 90,
  clockJumpSeconds: 60,
};

const UNSET: OperationsSettingsView = {
  version: null,
  values: DEFAULTS,
  defaults: DEFAULTS,
  setBy: null,
  setAt: null,
};

const POLICY: SecurityPolicyView = {
  version: 2,
  values: {
    idleTimeoutMinutes: 10,
    lockoutFailures: 5,
    lockoutMinutes: 15,
    allowSecurityKeyTouchOnly: false,
  },
  defaults: {
    idleTimeoutMinutes: 15,
    lockoutFailures: 5,
    lockoutMinutes: 15,
    allowSecurityKeyTouchOnly: false,
  },
  setBy: "Test Administrator",
  setAt: "2026-10-12T14:03:00Z",
};

const SEPARATION_DEFAULT: SeparationPolicyView = {
  version: null,
  values: {
    selfApproveConfiguration: false,
    selfReleaseReports: false,
    selfVerifyDestinations: false,
  },
  reason: null,
  setBy: null,
  setAt: null,
};

const ONE_PERSON: SeparationPolicyView = {
  version: 1,
  values: {
    selfApproveConfiguration: true,
    selfReleaseReports: true,
    selfVerifyDestinations: true,
  },
  reason: "Set up for one person: the director runs the laboratory alone.",
  setBy: "00000000-0000-7000-8000-0000000000e1",
  setAt: "2026-10-14T10:00:00Z",
};

function renderPage(capabilities: string[], commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({ current_session: () => sessionWith(capabilities), ...commands });
  return renderWithProviders(<OperationsSettingsPage />);
}

describe("OperationsSettingsPage (REQ-OPS-007, REQ-OPS-008, NFR-19)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  it("shows the defaults as defaults and sets the first version without a reason", async () => {
    const set = vi.fn(() => ({ ...UNSET, version: 1 }));
    const { user, container } = renderPage(["manage_configuration"], {
      get_operations_settings: () => UNSET,
      set_operations_settings: set,
    });
    expect(await screen.findByText("Defaults apply")).toBeTruthy();
    expect(screen.getByText("60 minutes (default)")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Security policy" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Change operations settings" }));
    const dialog = await screen.findByRole("dialog", { name: "Change operations settings" });
    expect(within(dialog).queryByRole("textbox", { name: /^Reason/ })).toBeNull();
    expect(
      within(dialog).getByText("In minutes. Default 60. The app accepts 5 to 1440."),
    ).toBeTruthy();
    const interval = within(dialog).getByRole("textbox", { name: /^Backup interval/ });
    await user.clear(interval);
    await user.type(interval, "1.5");
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await within(dialog).findByText("Enter a whole number.")).toBeTruthy();
    await user.clear(interval);
    await user.type(interval, "30");
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Operations settings saved")).toBeTruthy();
    expect(set).toHaveBeenCalledWith({
      input: {
        expectedVersion: null,
        values: { ...DEFAULTS, backupIntervalMinutes: 30 },
        reason: null,
      },
    });
  });

  it("shows a set policy and requires a reason to change it", async () => {
    const set = vi.fn(() => ({ ...POLICY, version: 3 }));
    const { user, container } = renderPage(["manage_users"], {
      get_security_policy: () => POLICY,
      set_security_policy: set,
    });
    expect(await screen.findByText("10 minutes")).toBeTruthy();
    expect(screen.getByText("5 attempts (default)")).toBeTruthy();
    expect(screen.getByText("2026-10-12 14:03 UTC")).toBeTruthy();
    expect(screen.queryByText("Defaults apply")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Operations settings", level: 2 })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(screen.getByRole("button", { name: "Change security policy" }));
    const dialog = await screen.findByRole("dialog", { name: "Change security policy" });
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(set).not.toHaveBeenCalled();
    await typeInto(user, /^Reason/, "Policy review");
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await screen.findByText("Security policy saved")).toBeTruthy();
    expect(set).toHaveBeenCalledWith({
      input: {
        expectedVersion: 2,
        idleTimeoutMinutes: 10,
        lockoutFailures: 5,
        lockoutMinutes: 15,
        allowSecurityKeyTouchOnly: false,
        reason: "Policy review",
        password: null,
      },
    });
  });

  it("shows the engine's field errors on the matching fields", async () => {
    const set = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            { field: "values.keepRecent", code: "out_of_range", message: "Keep 2 to 96 sets." },
          ],
        }),
      ),
    );
    const { user } = renderPage(["manage_configuration"], {
      get_operations_settings: () => UNSET,
      set_operations_settings: set,
    });
    await user.click(await screen.findByRole("button", { name: "Change operations settings" }));
    const dialog = await screen.findByRole("dialog", { name: "Change operations settings" });
    await user.click(within(dialog).getByRole("button", { name: "Save settings" }));
    expect(await within(dialog).findByText("Keep 2 to 96 sets.")).toBeTruthy();
    const field = within(dialog).getByRole("textbox", { name: /^Recent backup sets kept/ });
    expect(field.getAttribute("aria-invalid")).toBe("true");
  });

  it("shows the engine's error", async () => {
    renderPage(["manage_configuration"], {
      get_operations_settings: () =>
        Promise.reject(engineError("internal", "Settings unavailable.")),
    });
    expect(await screen.findByText("Settings unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("says when there is nothing to show", async () => {
    const { container } = renderPage([], {});
    expect(await screen.findByText("No settings to show")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  describe("separation of duties (ADR 0018)", () => {
    it("shows the current policy to quality staff without configuration controls", async () => {
      const set = vi.fn();
      renderPage(["manage_quality"], {
        get_separation_policy: () => SEPARATION_DEFAULT,
        set_separation_policy: set,
      });
      expect(
        await screen.findByText("Default: a second person is required for each."),
      ).toBeTruthy();
      expect(screen.queryByRole("button", { name: "Change separation of duties" })).toBeNull();
      expect(set).not.toHaveBeenCalled();
    });

    it("lets a configuration approver set a version with a reason and password", async () => {
      const set = vi.fn(() => ({ ...SEPARATION_DEFAULT, version: 1 }));
      const { user, container } = renderPage(["approve_configuration"], {
        get_separation_policy: () => SEPARATION_DEFAULT,
        set_separation_policy: set,
      });
      expect(
        await screen.findByText("Default: a second person is required for each."),
      ).toBeTruthy();
      expect(screen.getAllByText("Second person required")).toHaveLength(3);
      expect(screen.getByText(/let one qualified person hold every role/)).toBeTruthy();
      expect(screen.queryByRole("heading", { name: "Security policy" })).toBeNull();
      await expectNoA11yViolations(container, { wholePage: true });

      await user.click(screen.getByRole("button", { name: "Change separation of duties" }));
      const dialog = await screen.findByRole("dialog", { name: "Change separation of duties" });
      const configuration = within(dialog).getByRole("checkbox", {
        name: /Approve configuration they wrote/,
      });
      expect(configuration).toHaveProperty("checked", false);
      await user.click(configuration);
      await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
      await user.click(within(dialog).getByRole("button", { name: "Save policy" }));
      expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
      expect(set).not.toHaveBeenCalled();
      await typeInto(user, /^Reason/, "The director approves the procedures they write");
      await user.click(within(dialog).getByRole("button", { name: "Save policy" }));
      expect(await screen.findByText("Separation of duties saved")).toBeTruthy();
      expect(set).toHaveBeenCalledWith({
        input: {
          expectedVersion: null,
          values: {
            selfApproveConfiguration: true,
            selfReleaseReports: false,
            selfVerifyDestinations: false,
          },
          reason: "The director approves the procedures they write",
          password: "synthetic-secret",
        },
      });
    });

    it("shows a set version, and a wrong password on its field", async () => {
      const { user } = renderPage(["approve_configuration"], {
        get_separation_policy: () => ONE_PERSON,
        set_separation_policy: () =>
          Promise.reject(
            engineError("validation", "Check the highlighted fields.", {
              fieldErrors: [
                { field: "password", code: "incorrect", message: "Incorrect password." },
              ],
            }),
          ),
      });
      expect(
        await screen.findAllByText("One person may do both, confirming each time"),
      ).toHaveLength(3);
      expect(screen.getByText(ONE_PERSON.reason ?? "")).toBeTruthy();
      expect(screen.getByText(/2026-10-14 10:00 UTC/).closest("dd")?.textContent).toBe(
        "2026-10-14 10:00 UTC by you",
      );
      await user.click(screen.getByRole("button", { name: "Change separation of duties" }));
      const dialog = await screen.findByRole("dialog", { name: "Change separation of duties" });
      expect(
        within(dialog).getByRole("checkbox", { name: /Release a report whose results/ }),
      ).toHaveProperty("checked", true);
      await typeInto(user, /^Reason/, "A second technologist joined");
      await user.type(within(dialog).getByLabelText(/^Your password/), "wrong-secret");
      await user.click(within(dialog).getByRole("button", { name: "Save policy" }));
      expect(await within(dialog).findByText("Incorrect password.")).toBeTruthy();
      expect(
        within(dialog)
          .getByLabelText(/^Your password/)
          .getAttribute("aria-invalid"),
      ).toBe("true");
    });
  });
});
