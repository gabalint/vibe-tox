import { screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";
import { DetailPanel } from "@/components/DetailPanel";
import { openRecordPanel } from "./openRecordPanel";
import { renderWithProviders } from "./test/render";

const ROWS = [{ id: "SYN-1" }, { id: "SYN-2" }];
const render = (row: { id: string }) => (
  <DetailPanel identifier={row.id} onClose={vi.fn()}>
    <p>Synthetic record</p>
  </DetailPanel>
);
const show = (node: ReactNode) => renderWithProviders(<div>{node}</div>);

describe("openRecordPanel", () => {
  it("shows nothing while no record is open or the rows are loading", () => {
    const close = vi.fn();
    expect(openRecordPanel({ openId: null, close }, ROWS, (row) => row.id, render)).toBeNull();
    expect(
      openRecordPanel({ openId: "SYN-1", close }, undefined, (row) => row.id, render),
    ).toBeNull();
  });

  it("shows the open record from the loaded rows", () => {
    show(openRecordPanel({ openId: "SYN-2", close: vi.fn() }, ROWS, (row) => row.id, render));
    expect(screen.getByRole("region", { name: "SYN-2" })).toBeTruthy();
  });

  it("says when the record is not in the list, with the screen's own offer, and closes", async () => {
    const close = vi.fn();
    const { user } = show(
      openRecordPanel(
        { openId: "SYN-9", close },
        ROWS,
        (row) => row.id,
        render,
        <button type="button">Show all</button>,
      ),
    );
    const panel = screen.getByRole("region", { name: "Record not found" });
    expect(within(panel).getByText(/not in this list/)).toBeTruthy();
    expect(within(panel).getByRole("button", { name: "Show all" })).toBeTruthy();
    await user.click(within(panel).getByRole("button", { name: "Close" }));
    expect(close).toHaveBeenCalled();
  });
});
