import { act, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LoadedResultFile } from "@/generated/LoadedResultFile";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { FILE_INSTRUMENT, FILE_SOURCE } from "../fileTemplates/testTemplates";
import { IDLE_INSTRUMENT, INSTRUMENT, sessionWith } from "../testData";
import { ImportAttemptsPage } from "./ImportAttemptsPage";

const FILE_READINESS: InstrumentReadinessView = {
  ...FILE_INSTRUMENT,
  activeRegistration: {
    registrationId: FILE_SOURCE.registrationId,
    sender: "Synthetic Quant",
    version: "12.1",
    fileTemplate: true,
  },
};

const LOADED: LoadedResultFile = {
  receiptId: "00000000-0000-7000-8000-000000009101",
  instrumentId: FILE_INSTRUMENT.instrumentId,
  receivedAt: "2026-10-13T10:00:00Z",
  outcome: "received",
  duplicateOf: null,
  fileName: "SYN-01 batch 2026-10-13 0958.csv",
  byteLen: 48213,
  sha256: "a".repeat(64),
};

const ATTEMPT: ImportAttemptSummary = {
  importAttemptId: "00000000-0000-7000-8000-000000008101",
  receiptId: LOADED.receiptId,
  attemptNo: 1,
  instrumentId: FILE_INSTRUMENT.instrumentId,
  outcome: "processed",
  failureCode: null,
  rowsTotal: 5,
  rowsNew: 2,
  rowsDuplicate: 0,
  rowsReprocessed: 0,
  rowsQc: 1,
  rowsQuarantined: 1,
  rowsNotResult: 1,
  rowsSetAside: 0,
  rowsRetained: 0,
  processedAt: "2026-10-13T10:00:02Z",
};

function renderPage(
  capabilities: string[],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_instrument_readiness: () => [INSTRUMENT, IDLE_INSTRUMENT, FILE_READINESS],
    get_receipt_import_attempt: () => null,
    list_import_attempts: () => [],
    load_result_file: () => LOADED,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/review/imports", element: <ImportAttemptsPage /> }],
    "/review/imports",
  );
}

const BOTH = ["perform_analysis", "review_results"];

async function chooseFile(user: ReturnType<typeof renderPage>["user"], instrument: string) {
  await user.click(await screen.findByRole("button", { name: "Load results file" }));
  const dialog = await screen.findByRole("dialog", { name: "Load results file" });
  await chooseWithKeyboard(user, /Instrument$/, instrument);
  return dialog;
}

describe("Load results file (result files brief §6, §7)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);
  afterEach(() => {
    vi.useRealTimers();
  });

  it("loads a file for an instrument with a file template, then offers its import attempt", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const load = vi.fn(() => LOADED);
    const attempts = vi
      .fn<() => ImportAttemptSummary | null>()
      .mockReturnValueOnce(null)
      .mockReturnValue(ATTEMPT);
    const { user, router, container } = renderPage(BOTH, {
      load_result_file: load,
      get_receipt_import_attempt: attempts,
    });
    await user.click(await screen.findByRole("button", { name: "Load results file" }));
    const dialog = await screen.findByRole("dialog", { name: "Load results file" });
    // Only instruments whose approved source is a file template are offered.
    await user.click(within(dialog).getByRole("button", { name: /Instrument$/ }));
    const listbox = await screen.findByRole("listbox");
    expect(
      within(listbox)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual([FILE_INSTRUMENT.name]);
    await user.click(within(listbox).getByRole("option", { name: FILE_INSTRUMENT.name }));
    expect(
      await within(dialog).findByText("Read with the approved template of Synthetic Quant 12.1."),
    ).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    const received = await screen.findByRole("dialog", { name: "File received" });
    expect(within(received).getByText("SYN-01 batch 2026-10-13 0958.csv")).toBeTruthy();
    expect(within(received).getByText("48,213 bytes")).toBeTruthy();
    expect(load).toHaveBeenCalledWith({ input: { instrumentId: FILE_INSTRUMENT.instrumentId } });
    // The import follows from the outbox; the list is read again until it appears.
    await act(() => vi.advanceTimersByTimeAsync(1_000));
    await user.click(await within(received).findByRole("button", { name: "Open import attempt" }));
    expect(router.state.location.pathname).toBe(`/review/imports/${ATTEMPT.importAttemptId}`);
  });

  it("says a duplicate file was already received, and points to the earlier attempt", async () => {
    const { user } = renderPage(BOTH, {
      load_result_file: () => ({
        ...LOADED,
        outcome: "duplicate",
        duplicateOf: ATTEMPT.receiptId,
        receiptId: "r-2",
      }),
      get_receipt_import_attempt: () => ATTEMPT,
    });
    const dialog = await chooseFile(user, FILE_INSTRUMENT.name);
    await within(dialog).findByText(/Read with the approved template/);
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    const duplicate = await screen.findByRole("dialog", { name: "File already received" });
    expect(
      within(duplicate).getByText(/use Retry import on the earlier import attempt/),
    ).toBeTruthy();
    expect(
      await within(duplicate).findByRole("button", { name: "Open the earlier import attempt" }),
    ).toBeTruthy();
  });

  it("says a file over the size limit was not read", async () => {
    const { user } = renderPage(BOTH, {
      load_result_file: () => ({ ...LOADED, outcome: "too_large", sha256: null }),
    });
    const dialog = await chooseFile(user, FILE_INSTRUMENT.name);
    await within(dialog).findByText(/Read with the approved template/);
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    const tooLarge = await screen.findByRole("dialog", { name: "File not read" });
    expect(
      within(tooLarge).getByText("The file is larger than the 20 MiB the LIS reads"),
    ).toBeTruthy();
    expect(within(tooLarge).queryByRole("button", { name: /import attempt/ })).toBeNull();
  });

  it("keeps the dialog open when the file dialog is cancelled, and shows a refusal", async () => {
    const load = vi
      .fn()
      .mockReturnValueOnce(null)
      .mockRejectedValue(engineError("forbidden", "Loading files needs perform analysis."));
    const { user } = renderPage(BOTH, { load_result_file: load });
    const dialog = await chooseFile(user, FILE_INSTRUMENT.name);
    await within(dialog).findByText(/Read with the approved template/);
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    expect(
      await within(dialog).findByText("The file dialog was cancelled. No file was loaded."),
    ).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Choose file" }));
    expect(await within(dialog).findByText("Loading files needs perform analysis.")).toBeTruthy();
  });

  it("says when no instrument has an approved file template", async () => {
    const { user } = renderPage(BOTH, {
      list_instrument_readiness: () => [INSTRUMENT, IDLE_INSTRUMENT],
    });
    await user.click(await screen.findByRole("button", { name: "Load results file" }));
    const dialog = await screen.findByRole("dialog", { name: "Load results file" });
    expect(
      await within(dialog).findByText("No instrument has an approved file template yet."),
    ).toBeTruthy();
  });

  it("is offered only to people with perform_analysis", async () => {
    renderPage(["review_results"]);
    expect(await screen.findByText("No imports")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load results file" })).toBeNull();
  });
});
