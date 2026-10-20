import { screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { LABEL_QUEUE, NOW, REVIEW_QUEUE } from "./testData";
import { WorkQueuesPage } from "./WorkQueuesPage";

function renderPage(listQueues: () => unknown) {
  mockCommands({ list_work_queues: listQueues });
  return renderRoutes([{ path: "/work-queues", element: <WorkQueuesPage /> }], "/work-queues");
}

describe("WorkQueuesPage (REQ-OPS-004)", () => {
  beforeEach(() => {
    mockGridLayout();
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(NOW));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("lists the queues with counts and ages, and opens one with Enter", async () => {
    const list = vi.fn(() => [REVIEW_QUEUE, LABEL_QUEUE]);
    const { user, container } = renderPage(list);
    expect(await screen.findByRole("gridcell", { name: "Results ready for review" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({});
    expect(screen.getByRole("gridcell", { name: "3" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "0" })).toBeTruthy();
    expect(
      screen.getByRole("gridcell", { name: "3 h 20 min (since 2026-10-13 08:40 UTC)" }),
    ).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "—" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Reprint" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "3");
    expect(await screen.findByText("Navigated to /work-queues/review_ready")).toBeTruthy();
  });

  it("opens a queue with a single click, with no usage hint", async () => {
    const { user } = renderPage(() => [REVIEW_QUEUE, LABEL_QUEUE]);
    expect(await screen.findByRole("gridcell", { name: "Reprint" })).toBeTruthy();
    expect(screen.queryByText(/double-click/)).toBeNull();
    await openRow(user, "Reprint");
    expect(await screen.findByText("Navigated to /work-queues/label_failures")).toBeTruthy();
  });

  it("refreshes on request and says when the queues were read", async () => {
    const list = vi.fn(() => [REVIEW_QUEUE]);
    const { user } = renderPage(list);
    expect(await screen.findByText("Updated 12:00 UTC")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("labels a summary count that reached the engine bound", async () => {
    renderPage(() => [{ ...REVIEW_QUEUE, count: 500, countCapped: true }]);
    expect(await screen.findByRole("gridcell", { name: "500 or more" })).toBeTruthy();
  });

  it("shows uncounted historical review as unknown rather than zero", async () => {
    renderPage(() => [{ ...REVIEW_QUEUE, queue: "review_stale", count: null, oldestSince: null }]);
    expect(await screen.findByRole("gridcell", { name: "Open to check" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "0" })).toBeNull();
  });

  it("says when no queue applies", async () => {
    renderPage(() => []);
    expect(await screen.findByText("No work queues")).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
  });

  it("shows the engine's error", async () => {
    renderPage(() => Promise.reject(engineError("internal", "The work queues are unavailable.")));
    expect(await screen.findByText("The work queues are unavailable.")).toBeTruthy();
  });
});
