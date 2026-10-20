import { screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BillingWorkCursor } from "@/generated/BillingWorkCursor";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { SUBJECT, sessionWith, workEntry } from "../testData";
import { BillingWorkPage } from "./BillingWorkPage";

describe("Billing work continuation and exact patient/date scope (mocked IPC)", () => {
  beforeEach(mockGridLayout);

  it("uses all cursor fields to reach work beyond 200 rows at equal timestamps", async () => {
    const first = Array.from({ length: 200 }, (_, index) =>
      workEntry({
        subjectId: `subject-${index}`,
        accessionNumbers: [`A-${index}`],
        hasMore: true,
      }),
    );
    const last = first.at(-1);
    if (!last) throw new Error("Synthetic page is empty");
    const next = workEntry({
      subjectId: "subject-200",
      accessionNumbers: ["A-200"],
      hasMore: false,
    });
    const list = vi.fn((args: unknown) => {
      const { after } = (args as { input: { after: BillingWorkCursor | null } }).input;
      return after ? [next] : first;
    });
    mockCommands({ current_session: () => sessionWith(["billing"]), list_billing_work: list });
    const { user } = renderRoutes(
      [{ path: "/billing/work", element: <BillingWorkPage /> }],
      "/billing/work",
    );
    await user.click(await screen.findByRole("button", { name: "Load more billing work" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          filter: "needs_derivation",
          limit: 200,
          target: null,
          after: {
            since: last.since,
            subjectId: last.subjectId,
            dateOfService: last.dateOfService,
          },
        },
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Load more billing work" })).toBeNull(),
    );
    expect(screen.getByText(/201 billing work items/)).toBeTruthy();
  });

  it.each(["2026-10-11", ""])(
    "passes the exact queue collection date %s without substituting today's date",
    async (dateOfService) => {
      const list = vi.fn(() => []);
      mockCommands({ current_session: () => sessionWith(["billing"]), list_billing_work: list });
      const query = new URLSearchParams({
        filter: "needs_derivation",
        subjectId: SUBJECT.subjectId,
        dateOfService,
        open: `${SUBJECT.subjectId}|${dateOfService}`,
      });
      renderRoutes(
        [{ path: "/billing/work", element: <BillingWorkPage /> }],
        `/billing/work?${query}`,
      );
      await screen.findByText("Nothing in “Needs derivation”");
      expect(list).toHaveBeenCalledWith({
        input: {
          filter: "needs_derivation",
          limit: 200,
          after: null,
          target: { subjectId: SUBJECT.subjectId, dateOfService: dateOfService || null },
        },
      });
    },
  );
  it("makes queue target scope visible and clears it to restore every patient/date", async () => {
    const list = vi.fn(() => []);
    mockCommands({ current_session: () => sessionWith(["billing"]), list_billing_work: list });
    const { user, router } = renderRoutes(
      [{ path: "/billing/work", element: <BillingWorkPage /> }],
      `/billing/work?filter=needs_derivation&subjectId=${SUBJECT.subjectId}&dateOfService=2026-10-11&open=old-target`,
    );
    expect(await screen.findByText(/Showing the selected patient’s work for/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Show all patients and dates" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { filter: "needs_derivation", limit: 200, target: null, after: null },
      }),
    );
    expect(router.state.location.search).toBe("?filter=needs_derivation");
    expect(screen.queryByText(/Showing the selected patient’s work for/)).toBeNull();
  });
});
