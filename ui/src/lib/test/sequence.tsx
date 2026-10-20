import { screen } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import type { ReactNode } from "react";
import { expect } from "vitest";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { renderRoutes } from "./router";

interface RecordList {
  /** The record page's route, such as `/orders/:orderId`. */
  route: { path: string; element: ReactNode };
  recordPath: (id: string) => string;
  /** The list's records in display order; the first one opens. */
  ids: readonly string[];
}

/**
 * Opens the first of `ids` from a synthetic list titled "Synthetic list", as a list's row
 * would, so the record page shows its breadcrumb and Previous and Next through the list.
 */
export async function renderFromList({ route, recordPath, ids }: RecordList) {
  const result = renderRoutes(
    [{ path: "/list", element: <SyntheticList recordPath={recordPath} ids={ids} /> }, route],
    "/list",
  );
  await result.user.click(screen.getByRole("button", { name: "Open the first record" }));
  return result;
}

function SyntheticList({ recordPath, ids }: Omit<RecordList, "route">) {
  const open = useOpenRecordPage(recordPath, "Synthetic list");
  return (
    <button type="button" onClick={() => open(ids[0] ?? "", ids)}>
      Open the first record
    </button>
  );
}

/**
 * Presses Next and checks that focus stays on it once the next record, titled `nextTitle`,
 * has loaded (WCAG 2.4.3), rather than dropping to the page; also when that record is the
 * last one and Next becomes disabled.
 */
export async function expectNextKeepsFocus(user: UserEvent, nextTitle: string) {
  const next = screen.getByRole("link", { name: "Next" });
  await user.click(next);
  expect(await screen.findByRole("heading", { level: 1, name: nextTitle })).toBeTruthy();
  expect(document.activeElement).toBe(next);
}
