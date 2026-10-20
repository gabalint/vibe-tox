import { screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ListenerStatus, ListenerSummary } from "./ListenerStatus";

const ANALYZER: ListenerStatusView = {
  instrumentId: "00000000-0000-7000-8000-0000000000a1",
  instrumentName: "Synthetic Analyzer 1",
  state: "receiving",
  lastActivityAt: "2026-10-12T14:03:05Z",
  detail: null,
};

describe("ListenerStatus (REQ-OPS-013)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows each listener's state and last activity", async () => {
    mockCommands({
      list_listener_status: () => [
        ANALYZER,
        {
          ...ANALYZER,
          instrumentId: "00000000-0000-7000-8000-0000000000a2",
          instrumentName: "Synthetic Analyzer 2",
          state: "error",
          lastActivityAt: null,
          detail: "open_failed",
        },
      ],
    });
    const { container } = renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("Synthetic Analyzer 1")).toBeTruthy();
    expect(screen.getByText("Receiving")).toBeTruthy();
    expect(screen.getByText("2026-10-12 14:03 UTC")).toBeTruthy();
    expect(screen.getByText("Error")).toBeTruthy();
    expect(screen.getByText("Could not open the connection")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("says when a watched folder cannot be read or has files waiting (result files §6)", async () => {
    mockCommands({
      list_listener_status: () => [
        { ...ANALYZER, state: "error", detail: "folder_unavailable", filesWaiting: 0 },
        {
          ...ANALYZER,
          instrumentId: "00000000-0000-7000-8000-0000000000a2",
          instrumentName: "Synthetic LC-MS/MS 1",
          state: "listening",
          detail: "files_waiting",
          filesWaiting: 1,
        },
      ],
    });
    renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("The watched folder cannot be read")).toBeTruthy();
    expect(screen.getByText("1 file waiting to be read for more than 5 minutes")).toBeTruthy();
  });

  it("says when an instrument is loaded by hand only, or a file was too large to read", async () => {
    mockCommands({
      list_listener_status: () => [
        { ...ANALYZER, state: "manual_only", lastActivityAt: null },
        {
          ...ANALYZER,
          instrumentId: "00000000-0000-7000-8000-0000000000a2",
          instrumentName: "Synthetic LC-MS/MS 2",
          state: "listening",
          detail: "file_too_large",
          filesTooLarge: 2,
        },
      ],
    });
    renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("Manual loading only")).toBeTruthy();
    expect(
      screen.getByText(
        "2 files in the folder are larger than the 20 MiB the LIS reads and were not read",
      ),
    ).toBeTruthy();
  });

  it("says when no instruments are configured", async () => {
    mockCommands({ list_listener_status: () => [] });
    renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("No instruments configured")).toBeTruthy();
  });

  it("shows a neutral unavailable state when the status cannot be read", async () => {
    mockCommands({
      list_listener_status: () => Promise.reject(engineError("internal", "Not available.")),
    });
    renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("Status unavailable")).toBeTruthy();
  });

  it("asks for the status again every 10 seconds", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const list = vi
      .fn<() => ListenerStatusView[]>()
      .mockReturnValueOnce([{ ...ANALYZER, state: "listening" }])
      .mockReturnValue([ANALYZER]);
    mockCommands({ list_listener_status: list });
    renderWithProviders(<ListenerStatus />);
    expect(await screen.findByText("Listening")).toBeTruthy();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(await screen.findByText("Receiving")).toBeTruthy();
    expect(list).toHaveBeenCalledTimes(2);
  });
});

describe("ListenerSummary (REQ-OPS-013)", () => {
  it("counts the listeners by state, errors first, and lists each one on request", async () => {
    mockCommands({
      list_listener_status: () => [
        { ...ANALYZER, state: "listening", lastActivityAt: null },
        { ...ANALYZER, instrumentId: "00000000-0000-7000-8000-0000000000a2", state: "connected" },
        {
          ...ANALYZER,
          instrumentId: "00000000-0000-7000-8000-0000000000a3",
          instrumentName: "Synthetic Analyzer 3",
          state: "error",
          detail: "transport_failed",
        },
      ],
    });
    const { user, container } = renderWithProviders(<ListenerSummary />);
    const summary = await screen.findByRole("button", {
      name: "3 analyzers: 1 error, 1 connected, 1 listening",
    });
    await expectNoA11yViolations(container);
    await user.click(summary);
    const dialog = await screen.findByRole("dialog", { name: "Analyzer listeners" });
    expect(within(dialog).getByText("Synthetic Analyzer 3")).toBeTruthy();
    expect(within(dialog).getByText("Connection failed")).toBeTruthy();
    expect(within(dialog).getAllByText("2026-10-12 14:03 UTC")).toHaveLength(2);
  });

  it("names one analyzer in the singular", async () => {
    mockCommands({ list_listener_status: () => [ANALYZER] });
    renderWithProviders(<ListenerSummary />);
    expect(await screen.findByRole("button", { name: "1 analyzer: 1 receiving" })).toBeTruthy();
  });

  it("says when no instruments are configured", async () => {
    mockCommands({ list_listener_status: () => [] });
    renderWithProviders(<ListenerSummary />);
    expect(await screen.findByRole("button", { name: "No instruments configured" })).toBeTruthy();
  });

  // A failed read never shows the last known states as current.
  it("says the status is unavailable when it cannot be read", async () => {
    mockCommands({
      list_listener_status: () => Promise.reject(engineError("internal", "Not available.")),
    });
    renderWithProviders(<ListenerSummary />);
    expect(
      await screen.findByRole("button", { name: "Analyzers: status unavailable" }),
    ).toBeTruthy();
  });
});
