import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReviewQueueEntry } from "@/generated/ReviewQueueEntry";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { SUBJECT, sessionWith } from "../testData";
import { ReviewQueuePage } from "./ReviewQueuePage";

const ENTRY: ReviewQueueEntry = {
  subject: SUBJECT,
  since: "2026-10-12T10:00:00Z",
  specimenId: "00000000-0000-7000-8000-000000006001",
  accessionNumber: "A26000001",
  specimenTypeCode: "urine",
  lines: 3,
  pending: 1,
  stale: 0,
  blocked: 2,
};

function renderPage(path: string, listQueue: (args: unknown) => unknown) {
  mockCommands({
    current_session: () => sessionWith(["review_results"]),
    list_review_queue: async (args) => {
      const answer = await listQueue(args);
      return Array.isArray(answer) ? { items: answer, next: null, scanned: answer.length } : answer;
    },
  });
  return renderRoutes([{ path: "/review/queue", element: <ReviewQueuePage /> }], path);
}

describe("ReviewQueuePage (REQ-RES-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists specimens ready for review and opens one with Enter", async () => {
    const list = vi.fn(() => [ENTRY]);
    const { user, container } = renderPage("/review/queue", list);
    await screen.findByRole("gridcell", { name: "A26000001" });
    // The engine supplies the checked-candidate continuation independently of matches.
    expect(list).toHaveBeenCalledWith({ input: { filter: "ready", limit: 50, after: null } });
    // A count of zero is a plain 0, not a badge.
    expect(screen.getByRole("gridcell", { name: "0" })).toBeTruthy();
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "A26000001");
    expect(
      await screen.findByText(`Navigated to /review/specimens/${ENTRY.specimenId}`),
    ).toBeTruthy();
  });

  it("opens a specimen with a single click, passing the queue's order", async () => {
    const { user, router } = renderPage("/review/queue?filter=blocked", () => [ENTRY]);
    await openRow(user, "urine");
    expect(
      await screen.findByText(`Navigated to /review/specimens/${ENTRY.specimenId}`),
    ).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Result review", href: "/review/queue?filter=blocked" },
        ids: [ENTRY.specimenId],
      },
    });
  });

  it("says when more history remains unchecked, and refreshes on request", async () => {
    const rows = Array.from({ length: 50 }, (_, index) => ({
      ...ENTRY,
      specimenId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`,
      accessionNumber: `A26${String(index).padStart(6, "0")}`,
    }));
    const list = vi.fn(() => ({
      items: rows,
      next: { since: ENTRY.since, specimenId: "checked-cursor" },
      scanned: 500,
    }));
    const { user } = renderPage("/review/queue", list);
    const grid = await screen.findByRole("grid", { name: "Specimens to review" });
    expect(grid.getAttribute("aria-rowcount")).toBe("51");
    expect(screen.getByText("50 specimens shown")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Load more" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("keeps the filter in the address", async () => {
    const list = vi.fn(() => [ENTRY]);
    const { user, router } = renderPage("/review/queue", list);
    await screen.findByRole("gridcell", { name: "A26000001" });
    await chooseWithKeyboard(user, /Show$/, "Blocked");
    expect(router.state.location.search).toBe("?filter=blocked");
    expect(list).toHaveBeenLastCalledWith({
      input: { filter: "blocked", limit: 50, after: null },
    });
  });

  it("says when nothing matches", async () => {
    renderPage("/review/queue?filter=stale", () => []);
    expect(await screen.findByText("Nothing to review")).toBeTruthy();
  });

  it("shows the engine's error, with no retry when the roles do not allow review", async () => {
    renderPage("/review/queue", () =>
      Promise.reject(engineError("forbidden", "Review needs the review results capability.")),
    );
    expect(await screen.findByText("Review needs the review results capability.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "Result review" })).toBeTruthy();
  });
  it("uses the engine cursor, even when it differs from the last matching specimen", async () => {
    const rows = Array.from({ length: 50 }, (_, index) => ({
      ...ENTRY,
      specimenId: `specimen-${index}`,
      accessionNumber: `A-${index}`,
    }));
    const cursor = { since: ENTRY.since, specimenId: "last-checked-not-matched" };
    const list = vi
      .fn()
      .mockReturnValueOnce({ items: rows, next: cursor, scanned: 500 })
      .mockReturnValueOnce({ items: [{ ...ENTRY, specimenId: "last" }], next: null, scanned: 1 });
    const { user } = renderPage("/review/queue", list);
    const grid = await screen.findByRole("grid", { name: "Specimens to review" });
    await user.click(screen.getByRole("button", { name: "Load more" }));
    await vi.waitFor(() => expect(grid.getAttribute("aria-rowcount")).toBe("52"));
    expect(list).toHaveBeenLastCalledWith({ input: { filter: "ready", limit: 50, after: cursor } });
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });
});
