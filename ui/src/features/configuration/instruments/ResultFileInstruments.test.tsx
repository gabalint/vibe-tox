import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { InstrumentsPage } from "./InstrumentsPage";

/** A synthetic result-file instrument, loading files by hand (no folder yet). */
const FILES: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001003",
  code: "SYN-MS1",
  name: "Synthetic LC-MS/MS 1",
  connection: { kind: "result_file" },
  enabled: true,
  revision: 2,
};

const WATCHED: InstrumentConfiguration = {
  ...FILES,
  resultFolder: "/srv/exports/syn-ms1",
  revision: 3,
};

const FOLDER_STATUS: ListenerStatusView = {
  instrumentId: FILES.id,
  instrumentName: FILES.name,
  state: "listening",
  lastActivityAt: null,
  detail: null,
};

function renderPage(
  instruments: InstrumentConfiguration[],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_instruments: () => instruments,
    list_listener_status: () => [FOLDER_STATUS],
    ...commands,
  });
  return renderRoutes([{ path: "/instruments", element: <InstrumentsPage /> }], "/instruments");
}

async function openInstrument(user: ReturnType<typeof renderPage>["user"]) {
  await openRow(user, FILES.code);
  return screen.findByRole("region", { name: FILES.code });
}

describe("Result-file instruments (result files brief §6, §7)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("registers a result-file instrument with no network or serial settings", async () => {
    const register = vi.fn(() => ({
      instrument: FILES,
      listenersRestarted: true,
      message: "Saved.",
    }));
    const { user } = renderPage([], { register_instrument: register });
    await user.click(await screen.findByRole("button", { name: "Register analyzer" }));
    const dialog = await screen.findByRole("dialog", { name: "Register analyzer" });
    await typeInto(user, /^Code/, "SYN-MS1");
    await typeInto(user, /^Name/, "Synthetic LC-MS/MS 1");
    await chooseWithKeyboard(user, /Connection$/, "Result files");
    expect(within(dialog).getByText(/exports result files/)).toBeTruthy();
    expect(within(dialog).queryByRole("textbox", { name: /Port/ })).toBeNull();
    expect(within(dialog).queryByRole("textbox", { name: /Serial port/ })).toBeNull();
    await chooseWithKeyboard(user, /Folder pickup$/, "Pick up files once a folder is chosen");
    await user.click(within(dialog).getByRole("button", { name: "Register analyzer" }));
    expect(await screen.findByText("Synthetic LC-MS/MS 1 registered")).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: {
        code: "SYN-MS1",
        name: "Synthetic LC-MS/MS 1",
        connection: { kind: "result_file" },
        enabled: true,
      },
    });
  });

  it("chooses the watched folder, saying files already there are not loaded", async () => {
    const choose = vi
      .fn()
      .mockReturnValueOnce(null)
      .mockReturnValue({ instrument: WATCHED, presentFiles: 0 });
    const { user, container } = renderPage([FILES], { choose_result_folder: choose });
    await screen.findByRole("gridcell", { name: "Result files" });
    const panel = await openInstrument(user);
    expect(
      within(panel).getByText("None. Files are loaded by hand under Result imports."),
    ).toBeTruthy();
    expect(within(panel).queryByRole("button", { name: "Clear folder" })).toBeNull();
    await expectNoA11yViolations(container);
    await user.click(within(panel).getByRole("button", { name: "Choose folder" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Choose the watched folder of Synthetic LC-MS/MS 1",
    });
    expect(
      within(confirm).getByText(/Files already there are not loaded automatically\./),
    ).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Choose folder" }));
    expect(
      await within(confirm).findByText("The folder dialog was cancelled. No folder was chosen."),
    ).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Choose folder" }));
    expect(await screen.findByText("Watched folder of Synthetic LC-MS/MS 1 saved")).toBeTruthy();
    expect(choose).toHaveBeenLastCalledWith({
      input: { instrumentId: FILES.id, expectedRevision: 2 },
    });
  });

  it("says how many files were already in the chosen folder and that they are not loaded", async () => {
    const { user } = renderPage([FILES], {
      choose_result_folder: () => ({ instrument: WATCHED, presentFiles: 3 }),
    });
    const panel = await openInstrument(user);
    await user.click(within(panel).getByRole("button", { name: "Choose folder" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Choose folder" }));
    const saved = await screen.findByRole("dialog", {
      name: "Watched folder of Synthetic LC-MS/MS 1 saved",
    });
    expect(
      within(saved).getByText(
        "3 files were already in this folder. They will not be loaded automatically; load any of them by hand with Load results file.",
      ),
    ).toBeTruthy();
    await user.click(within(saved).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows the watched folder with its pickup state, and clears it", async () => {
    const clear = vi.fn(() => FILES);
    const { user } = renderPage([WATCHED], {
      list_listener_status: () => [{ ...FOLDER_STATUS, detail: "files_waiting", filesWaiting: 3 }],
      clear_result_folder: clear,
    });
    await screen.findByRole("gridcell", { name: FILES.code });
    const menu = within(await openRowActions(user, FILES.code));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Edit connection",
      "Change folder",
      "Clear folder",
      "Disable folder pickup",
    ]);
    await user.keyboard("{Escape}");
    const panel = await openInstrument(user);
    expect(within(panel).getByText("/srv/exports/syn-ms1")).toBeTruthy();
    expect(
      await within(panel).findByText("3 files waiting to be read for more than 5 minutes"),
    ).toBeTruthy();
    await user.click(within(panel).getByRole("button", { name: "Clear folder" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Stop watching the folder of Synthetic LC-MS/MS 1?",
    });
    await user.click(within(confirm).getByRole("button", { name: "Clear folder" }));
    expect(await screen.findByText("Synthetic LC-MS/MS 1 no longer watches a folder")).toBeTruthy();
    expect(clear).toHaveBeenCalledWith({ input: { instrumentId: FILES.id, expectedRevision: 3 } });
  });

  it("says when the watched folder cannot be read, or is being watched", async () => {
    const { user } = renderPage([WATCHED], {
      list_listener_status: () => [
        { ...FOLDER_STATUS, state: "error", detail: "folder_unavailable" },
      ],
    });
    const panel = await openInstrument(user);
    expect(await within(panel).findByText("The watched folder cannot be read")).toBeTruthy();
  });

  it("disables folder pickup in words for files", async () => {
    const setEnabled = vi.fn(() => ({
      instrument: { ...WATCHED, enabled: false },
      listenersRestarted: true,
      message: "Saved.",
    }));
    const { user } = renderPage([WATCHED], { set_instrument_enabled: setEnabled });
    const panel = await openInstrument(user);
    expect(await within(panel).findByText("Watching")).toBeTruthy();
    // Beyond the first three actions, it is under More.
    await user.click(within(panel).getByRole("button", { name: "More" }));
    await user.click(await screen.findByRole("menuitem", { name: "Disable folder pickup" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Disable folder pickup for Synthetic LC-MS/MS 1?",
    });
    expect(within(confirm).getByText(/Files can still be loaded by hand\./)).toBeTruthy();
    await user.click(within(confirm).getByRole("button", { name: "Disable folder pickup" }));
    expect(setEnabled).toHaveBeenCalledWith({
      input: { instrumentId: FILES.id, expectedRevision: 3, enabled: false },
    });
  });
});
