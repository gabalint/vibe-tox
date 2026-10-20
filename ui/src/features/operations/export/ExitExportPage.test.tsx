import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ExitExportView } from "@/generated/ExitExportView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { JOB_POLL_MS } from "../useJobQuery";
import { ExitExportPage } from "./ExitExportPage";

const DONE: ExitExportView = {
  exportId: "00000000-0000-7000-8000-0000000000a1",
  state: "succeeded",
  requestedBy: "Test Operator",
  requestedAt: "2026-10-13T12:00:00Z",
  finishedAt: "2026-10-13T12:04:00Z",
  folderLabel: "Encrypted drive E",
  packageName: "vibe-tox-exit-synthetic-20261013T120000Z",
  databaseBytes: 52_428_800,
  artifactCount: 12,
  manifestSha256: "cd".repeat(32),
  failureCode: null,
};

const REQUESTED: ExitExportView = {
  ...DONE,
  exportId: "00000000-0000-7000-8000-0000000000a2",
  state: "requested",
  requestedAt: "2026-10-13T15:00:00Z",
  finishedAt: null,
  folderLabel: "Encrypted drive F",
  packageName: null,
  databaseBytes: null,
  artifactCount: null,
  manifestSha256: null,
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["export"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_exit_exports: () => [DONE],
    ...commands,
  });
  return renderWithProviders(<ExitExportPage />);
}

async function openExport(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Export all data" }));
  const dialog = await screen.findByRole("alertdialog", { name: "Export all data" });
  await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Moving to a new LIS");
  await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
  return dialog;
}

describe("ExitExportPage (REQ-OPS-010)", () => {
  beforeEach(mockGridLayout);

  it("lists exports and requests one after the plaintext warning", async () => {
    const request = vi.fn(() => REQUESTED);
    const list = vi.fn(() => [DONE]);
    const { user, container } = renderPage({
      request_exit_export: request,
      list_exit_exports: list,
    });
    expect(await screen.findByText("vibe-tox-exit-synthetic-20261013T120000Z")).toBeTruthy();
    expect(screen.getByText("Verified")).toBeTruthy();
    expect(screen.getByText("50.0 MiB")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { limit: 101 } });
    await expectNoA11yViolations(container, { wholePage: true });
    const dialog = await openExport(user);
    expect(within(dialog).getByText("The package is not encrypted")).toBeTruthy();
    expect(within(dialog).getByText(/Save it only to encrypted media/)).toBeTruthy();
    expect(within(dialog).getByText(/a data dictionary and an independent verifier/)).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Choose the folder" }));
    expect(await screen.findByText("Exit export requested to Encrypted drive F")).toBeTruthy();
    expect(request).toHaveBeenCalledWith({
      input: { reason: "Moving to a new LIS", password: "synthetic-secret" },
    });
  });

  it("keeps the dialog open when no folder is chosen", async () => {
    const { user } = renderPage({ request_exit_export: () => null });
    const dialog = await openExport(user);
    await user.click(within(dialog).getByRole("button", { name: "Choose the folder" }));
    expect(
      await within(dialog).findByText("No folder was chosen, so nothing was exported."),
    ).toBeTruthy();
  });

  it("shows a wrong password on its field", async () => {
    const wrong = engineError("validation", "The password is incorrect.", {
      fieldErrors: [
        { field: "password", code: "incorrect", message: "The password is incorrect." },
      ],
    });
    const { user } = renderPage({ request_exit_export: () => Promise.reject(wrong) });
    const dialog = await openExport(user);
    await user.click(within(dialog).getByRole("button", { name: "Choose the folder" }));
    expect(await within(dialog).findByText("Not exported")).toBeTruthy();
    const password = within(dialog).getByLabelText(/^Your password/);
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(within(dialog).getAllByText("The password is incorrect.").length).toBeGreaterThan(0);
    expect(screen.getByRole("alertdialog", { name: "Export all data" })).toBeTruthy();
  });

  it("says when older exports exist beyond the list", async () => {
    const many = Array.from({ length: 101 }, (_, index) => ({
      ...DONE,
      exportId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`,
    }));
    renderPage({ list_exit_exports: () => many });
    expect(await screen.findByText(/^Showing the newest results; sorting/)).toBeTruthy();
    expect(screen.getByRole("grid").getAttribute("aria-rowcount")).toBe("101");
  });

  it("asks again while an export runs and stops once it has finished", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const answers = [
        [REQUESTED],
        [{ ...REQUESTED, state: "running" }],
        [{ ...DONE, exportId: REQUESTED.exportId, folderLabel: "Encrypted drive F" }],
      ];
      const list = vi.fn(() => (answers.length > 1 ? answers.shift() : answers[0]));
      renderPage({ list_exit_exports: list });
      expect(await screen.findByRole("gridcell", { name: "Requested" })).toBeTruthy();
      await vi.advanceTimersByTimeAsync(JOB_POLL_MS);
      expect(await screen.findByRole("gridcell", { name: "Running" })).toBeTruthy();
      await vi.advanceTimersByTimeAsync(JOB_POLL_MS);
      expect(await screen.findByRole("gridcell", { name: "Verified" })).toBeTruthy();
      const asked = list.mock.calls.length;
      await vi.advanceTimersByTimeAsync(JOB_POLL_MS * 3);
      expect(list).toHaveBeenCalledTimes(asked);
    } finally {
      vi.useRealTimers();
    }
  });

  it("offers no export without the export capability", async () => {
    renderPage({}, []);
    expect(await screen.findByText("Encrypted drive E")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Export all data" })).toBeNull();
  });

  it("explains that nothing was exported yet", async () => {
    const { container } = renderPage({ list_exit_exports: () => [] });
    expect(await screen.findByText("No exit exports")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's error with its support reference", async () => {
    renderPage({
      list_exit_exports: () => Promise.reject(engineError("internal", "Exports are unavailable.")),
    });
    expect(await screen.findByText("Exports are unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
