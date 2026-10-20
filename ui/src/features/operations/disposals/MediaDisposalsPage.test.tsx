import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { MediaDisposalView } from "@/generated/MediaDisposalView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { MediaDisposalsPage } from "./MediaDisposalsPage";

const DISPOSAL: MediaDisposalView = {
  disposalId: "00000000-0000-7000-8000-0000000000d1",
  medium: "Removable backup drive 2",
  method: "destroyed",
  identifier: "SYN-SERIAL-0001",
  recordedBy: "Test Operator",
  recordedAt: "2026-10-13T11:00:00Z",
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["backup_restore"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_media_disposals: () => [DISPOSAL],
    ...commands,
  });
  return renderWithProviders(<MediaDisposalsPage />);
}

describe("MediaDisposalsPage (REQ-OPS-003)", () => {
  beforeEach(mockGridLayout);

  it("lists disposals and records one without an identifier", async () => {
    const record = vi.fn(() => ({ ...DISPOSAL, medium: "Old host disk", method: "wiped" }));
    const { user, container } = renderPage({ record_media_disposal: record });
    expect(await screen.findByText("Removable backup drive 2")).toBeTruthy();
    expect(screen.getByText("Destroyed")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(screen.getByRole("button", { name: "Record disposal" }));
    const dialog = await screen.findByRole("dialog", { name: "Record disposal" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Medium/ }), "Old host disk");
    await chooseWithKeyboard(user, /Method$/, "Wiped");
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Wiped by vendor");
    await user.click(within(dialog).getByRole("button", { name: "Record disposal" }));
    expect(await screen.findByText("Disposal recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        medium: "Old host disk",
        method: "wiped",
        identifier: null,
        note: "Wiped by vendor",
      },
    });
  });

  it("checks the required fields before recording", async () => {
    const record = vi.fn(() => null);
    const { user } = renderPage({ record_media_disposal: record });
    await user.click(await screen.findByRole("button", { name: "Record disposal" }));
    const dialog = await screen.findByRole("dialog", { name: "Record disposal" });
    await user.click(within(dialog).getByRole("button", { name: "Record disposal" }));
    expect(await within(dialog).findByText("Enter medium.")).toBeTruthy();
    expect(within(dialog).getByText("Choose method.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
  });

  it("shows the engine's refusal and offers no recording without backup and restore", async () => {
    const { container } = renderPage(
      {
        list_media_disposals: () =>
          Promise.reject(engineError("forbidden", "You do not have permission for this action.")),
      },
      [],
    );
    expect(await screen.findByText("You do not have permission for this action.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record disposal" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("explains an empty list", async () => {
    const { container } = renderPage({ list_media_disposals: () => [] });
    expect(await screen.findByText("No disposals recorded")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's error with its support reference", async () => {
    renderPage({
      list_media_disposals: () =>
        Promise.reject(engineError("internal", "Disposals are unavailable.")),
    });
    expect(await screen.findByText("Disposals are unavailable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
