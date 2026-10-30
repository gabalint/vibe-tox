import { screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { item, LABEL_QUEUE, NOW, REVIEW_QUEUE } from "./testData";
import { WorkQueuePage } from "./WorkQueuePage";

const SPECIMEN = item();
const LABEL_JOB = item({
  queue: "label_failures",
  itemKind: "outbox_item",
  itemId: "00000000-0000-7000-8000-0000000000a2",
  title: "A26000002 label",
  state: "failed",
  stateLabel: undefined,
  since: "2026-10-13T11:01:00Z",
  nextAction: "Reprint",
});
const ACCESSION = item({
  queue: "label_failures",
  itemKind: "accession",
  itemId: "00000000-0000-7000-8000-0000000000a3",
  title: "A26000003",
  nextAction: "Reprint",
});

function renderPage(path: string, listItems: (args: unknown) => unknown) {
  mockCommands({
    list_work_queues: () => [REVIEW_QUEUE, LABEL_QUEUE],
    list_work_queue_items: async (args) => {
      const answer = await listItems(args);
      return Array.isArray(answer) ? { items: answer, next: null, scanned: 0 } : answer;
    },
  });
  return renderRoutes([{ path: "/work-queues/:queue", element: <WorkQueuePage /> }], path);
}

describe("WorkQueuePage (REQ-OPS-004)", () => {
  beforeEach(() => {
    mockGridLayout();
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date(NOW));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("lists the queue's items with their ages and opens a specimen for review", async () => {
    const list = vi.fn(() => [SPECIMEN]);
    const { user, container, router } = renderPage("/work-queues/review_ready", list);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Results ready for review" }),
    ).toBeTruthy();
    expect(await screen.findByRole("gridcell", { name: "A26000001-urine" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: { queue: "review_ready", limit: 200, after: null },
    });
    expect(
      screen.getByRole("gridcell", { name: "2 d 4 h (since 2026-10-11 08:00 UTC)" }),
    ).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Ready" })).toBeTruthy();
    expect(screen.queryByText(/double-click/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "Ready");
    expect(
      await screen.findByText(`Navigated to /review/specimens/${SPECIMEN.itemId}`),
    ).toBeTruthy();
    // The record page's breadcrumb returns to this queue.
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Results ready for review", href: "/work-queues/review_ready" },
        ids: [SPECIMEN.itemId],
      },
    });
  });

  it("opens an item with a single click, and a label job not at all", async () => {
    const { user } = renderPage("/work-queues/label_failures", () => [LABEL_JOB, ACCESSION]);
    expect(
      await screen.findByRole("gridcell", { name: "59 min (since 2026-10-13 11:01 UTC)" }),
    ).toBeTruthy();
    // A state the engine sent without a label shows as sent.
    expect(screen.getByRole("gridcell", { name: "failed" })).toBeTruthy();
    // A label job has no screen: opening it stays here.
    await openRow(user, "failed");
    expect(screen.getByRole("heading", { level: 1, name: "Label print failures" })).toBeTruthy();
    await openRow(user, "A26000003");
    expect(await screen.findByText(`Navigated to /accessions/${ACCESSION.itemId}`)).toBeTruthy();
  });

  it("opens a QC item at its assessment and exposes a deviation target", async () => {
    const qc = item({
      queue: "qc_review",
      itemKind: "run_analyte_qc",
      itemId: "00000000-0000-7000-8000-0000000000c1:00000000-0000-7000-8000-0000000000c2",
      title: "Run R-1 / analyte AMP",
    });
    const deviation = item({
      queue: "qc_review",
      itemKind: "deviation",
      itemId: "00000000-0000-7000-8000-0000000000c3",
      title: "Deviation 00000000-0000-7000-8000-0000000000c3",
    });
    const { user } = renderPage("/work-queues/qc_review", () => [qc, deviation]);
    const deviationTitle = await screen.findByText(deviation.title);
    expect(deviationTitle.className).not.toMatch(/noScreen/);
    expect(screen.getByText(qc.title).className).not.toMatch(/noScreen/);
    await openRow(user, qc.title);
    expect(
      await screen.findByText(
        "Navigated to /bench/runs/00000000-0000-7000-8000-0000000000c1/qc?open=00000000-0000-7000-8000-0000000000c2",
      ),
    ).toBeTruthy();
  });

  it("opens a deviation directly without requiring a run ID", async () => {
    const deviation = item({
      itemKind: "deviation",
      title: "Shared lot deviation",
      itemId: "00000000-0000-7000-8000-0000000000c3",
    });
    const { user } = renderPage("/work-queues/open_deviations", () => [deviation]);
    await openRow(user, deviation.title);
    expect(
      await screen.findByText(`Navigated to /quality/deviations/${deviation.itemId}`),
    ).toBeTruthy();
  });

  it("passes Previous and Next through the queue's items of the opened item's kind", async () => {
    const derivation = (id: string, title: string) =>
      item({ queue: "billing_work", itemKind: "billing_derivation", itemId: id, title });
    const first = derivation("00000000-0000-7000-8000-0000000000d1", "A26000011");
    const second = derivation("00000000-0000-7000-8000-0000000000d2", "A26000012");
    const subject = item({
      queue: "billing_work",
      itemKind: "billing_subject",
      itemId: "00000000-0000-7000-8000-0000000000d3",
      title: "A26000013",
      state: "needs_derivation",
    });
    const { user, router } = renderPage("/work-queues/billing_work", () => [
      first,
      subject,
      second,
    ]);
    await openRow(user, "A26000012");
    expect(
      await screen.findByText(`Navigated to /billing/derivations/${second.itemId}`),
    ).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Work queue", href: "/work-queues/billing_work" },
        ids: [first.itemId, second.itemId],
      },
    });
  });

  it("says how many wait when the list stops at the limit", async () => {
    const many = Array.from({ length: 200 }, (_, index) =>
      item({ itemId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}` }),
    );
    mockCommands({
      list_work_queues: () => [{ ...REVIEW_QUEUE, count: 340 }],
      list_work_queue_items: () => ({ items: many, next: null, scanned: 0 }),
    });
    renderRoutes(
      [{ path: "/work-queues/:queue", element: <WorkQueuePage /> }],
      "/work-queues/review_ready",
    );
    expect(await screen.findByText("200 of 340 items")).toBeTruthy();
    expect(screen.getByText(/Showing the oldest results; sorting/)).toBeTruthy();
  });

  it("shows each state in the engine's words, and one without them as sent", async () => {
    const held = item({
      itemId: "00000000-0000-7000-8000-0000000000b1",
      title: "A1",
      state: "held",
      stateLabel: "Held",
    });
    const gate = item({
      itemId: "00000000-0000-7000-8000-0000000000b2",
      title: "A2",
      state: "specimen_held",
      stateLabel: undefined,
    });
    renderPage("/work-queues/review_ready", () => [held, gate]);
    expect(await screen.findByRole("gridcell", { name: "Held" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "specimen_held" })).toBeTruthy();
  });

  it("shows an item of an unknown kind with its next action and no link", async () => {
    const unknown = item({
      itemKind: "outbox_message",
      title: "Message 7",
      nextAction: "Investigate",
    });
    renderPage("/work-queues/outbox_dead_letters", () => [unknown]);
    expect(await screen.findByRole("heading", { level: 1, name: "Work queue" })).toBeTruthy();
    expect(await screen.findByRole("gridcell", { name: "Investigate" })).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Message 7" })).toBeNull();
  });

  it("says when the queue is empty", async () => {
    renderPage("/work-queues/review_ready", () => []);
    expect(await screen.findByText("Nothing waiting")).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
    // The breadcrumb, not a button, leads back to the overview.
    expect(screen.getByRole("link", { name: "Work queues" }).getAttribute("href")).toBe(
      "/work-queues",
    );
    expect(screen.queryByRole("button", { name: "All queues" })).toBeNull();
  });

  it("shows the engine's error", async () => {
    renderPage("/work-queues/review_ready", () =>
      Promise.reject(engineError("forbidden", "This queue needs the review results capability.")),
    );
    expect(await screen.findByText("This queue needs the review results capability.")).toBeTruthy();
  });
});
