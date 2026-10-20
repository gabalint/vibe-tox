import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DataGrid } from "@/components/DataGrid";
import { allowedActionItems, allowedRowActions, type OfferedAction } from "./allowedActions";
import { openRowActions } from "./test/grid";
import { mockGridLayout } from "./test/layout";
import { renderWithProviders } from "./test/render";

type Command = "verify_destination" | "replace_destination" | "retire_destination";

const ACTIONS: readonly OfferedAction<Command>[] = [
  { id: "verify_destination", label: "Verify" },
  { id: "replace_destination", label: "Replace" },
  { id: "retire_destination", label: "Retire", variant: "danger" },
];

interface Row {
  id: string;
  allowedActions: string[];
}

const ROWS: Row[] = [
  { id: "SYN-D1", allowedActions: ["retire_destination", "unknown_command", "verify_destination"] },
  { id: "SYN-D2", allowedActions: [] },
];

describe("allowedActionItems", () => {
  it("keeps the allowed actions in the screen's order, with their variant and handler", () => {
    const onAction = vi.fn();
    const items = allowedActionItems(ACTIONS, ROWS[0]?.allowedActions ?? [], onAction);
    expect(items.map(({ id, label, variant }) => ({ id, label, variant }))).toEqual([
      { id: "verify_destination", label: "Verify", variant: undefined },
      { id: "retire_destination", label: "Retire", variant: "danger" },
    ]);
    items[1]?.onAction();
    expect(onAction).toHaveBeenCalledWith("retire_destination");
  });
});

describe("allowedRowActions", () => {
  beforeEach(mockGridLayout);

  it("gives a grid the same menu, and no button to a row with nothing allowed", async () => {
    const onAction = vi.fn();
    const rowActions = allowedRowActions<Row, Command>(ACTIONS, {
      label: (row) => row.id,
      allowed: (row) => row.allowedActions,
      onAction,
    });
    const { user } = renderWithProviders(
      <DataGrid
        label="Synthetic destinations"
        columns={[{ id: "id", header: "Label", cell: (row: Row) => row.id }]}
        rows={ROWS}
        getRowId={(row) => row.id}
        rowActions={rowActions}
      />,
    );
    expect(screen.queryByRole("button", { name: "Actions for SYN-D2" })).toBeNull();
    const menu = await openRowActions(user, "SYN-D1");
    const items = within(menu).getAllByRole("menuitem");
    expect(items.map((item) => item.textContent)).toEqual(["Verify", "Retire"]);
    await user.click(within(menu).getByRole("menuitem", { name: "Retire" }));
    expect(onAction).toHaveBeenCalledWith("retire_destination", ROWS[0]);
  });
});
