import { screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { DataGrid, type DataGridColumn } from "./DataGrid";

interface Item {
  id: string;
  status: string;
}

const COLUMNS: readonly DataGridColumn<Item>[] = [
  { id: "id", header: "Accession", cell: (item) => item.id, sortValue: (item) => item.id },
  { id: "status", header: "Status", cell: (item) => item.status },
];

const items = (count: number): Item[] =>
  Array.from({ length: count }, (_, index) => ({
    id: `SYN-${String(count - index).padStart(6, "0")}`,
    status: index % 2 === 0 ? "Received" : "On hold",
  }));

const getRowId = (item: Item) => item.id;

function Harness({
  rows,
  onChange,
}: {
  rows: Item[];
  onChange?: (ids: ReadonlySet<string>) => void;
}) {
  const [selectedIds, setSelectedIds] = useState<ReadonlySet<string>>(new Set());
  return (
    <DataGrid
      label="Synthetic worklist"
      columns={COLUMNS}
      rows={rows}
      getRowId={getRowId}
      bulk={{
        selectedIds,
        onChange: (ids) => {
          setSelectedIds(ids);
          onChange?.(ids);
        },
        actions: <button type="button">Hand off</button>,
      }}
    />
  );
}

const dataRows = () => screen.getAllByRole("row").slice(1);
const firstAccession = () =>
  within(dataRows()[0] as HTMLElement).getAllByRole("gridcell")[1]?.textContent;

describe("DataGrid", () => {
  beforeEach(mockGridLayout);

  it("exposes grid semantics with row and column counts", async () => {
    const { container } = renderWithProviders(<Harness rows={items(3)} />);
    const grid = screen.getByRole("grid", { name: "Synthetic worklist" });
    expect(grid.getAttribute("aria-rowcount")).toBe("4");
    expect(grid.getAttribute("aria-colcount")).toBe("3");
    expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
      "Select all rows",
      "Accession",
      "Status",
    ]);
    await expectNoA11yViolations(container);
  });

  it("sorts by a sortable column on click and on Enter", async () => {
    const { user } = renderWithProviders(<Harness rows={items(3)} />);
    const header = screen.getByRole("columnheader", { name: "Accession" });
    expect(header.getAttribute("aria-sort")).toBe("none");
    expect(firstAccession()).toBe("SYN-000003");
    await user.click(header);
    expect(header.getAttribute("aria-sort")).toBe("ascending");
    expect(firstAccession()).toBe("SYN-000001");
    await user.keyboard("{Enter}");
    expect(header.getAttribute("aria-sort")).toBe("descending");
    expect(screen.getByRole("columnheader", { name: "Status" }).hasAttribute("aria-sort")).toBe(
      false,
    );
  });

  it("has one tab stop and moves focus with the arrow keys", async () => {
    const { user } = renderWithProviders(<Harness rows={items(3)} />);
    await user.tab();
    expect(document.activeElement?.textContent).toBe("Select all rows");
    await user.keyboard("{ArrowRight}{ArrowDown}");
    expect(document.activeElement?.textContent).toBe("SYN-000003");
    await user.keyboard("{End}");
    expect(document.activeElement?.textContent).toBe("Received");
    await user.keyboard("{Control>}{End}{/Control}");
    expect(document.activeElement?.textContent).toBe("Received");
    expect(document.activeElement?.closest("[role=row]")?.getAttribute("aria-rowindex")).toBe("4");
    await user.tab();
    expect(document.activeElement).toBe(document.body);
  });

  it("selects rows with Space and all rows from the header", async () => {
    const onChange = vi.fn();
    const { user } = renderWithProviders(<Harness rows={items(3)} onChange={onChange} />);
    await user.tab();
    await user.keyboard("{ArrowDown}{ArrowRight} ");
    expect(dataRows().map((row) => row.getAttribute("aria-selected"))).toEqual([
      "true",
      "false",
      "false",
    ]);
    expect([...(onChange.mock.lastCall?.[0] ?? [])]).toEqual(["SYN-000003"]);
    await user.keyboard("{Control>}{Home}{/Control} ");
    expect(dataRows().every((row) => row.getAttribute("aria-selected") === "true")).toBe(true);
    await user.keyboard(" ");
    expect(dataRows().every((row) => row.getAttribute("aria-selected") === "false")).toBe(true);
  });

  it("does not select a row when a data cell is clicked", async () => {
    const onChange = vi.fn();
    const { user } = renderWithProviders(<Harness rows={items(3)} onChange={onChange} />);
    await user.click(screen.getByRole("gridcell", { name: "SYN-000002" }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("renders every row up to 100 rows", () => {
    renderWithProviders(<Harness rows={items(100)} />);
    expect(dataRows()).toHaveLength(100);
  });

  it("renders only a window of rows above 100 and reaches the last row by keyboard", async () => {
    const { user } = renderWithProviders(<Harness rows={items(10_000)} />);
    expect(screen.getByRole("grid").getAttribute("aria-rowcount")).toBe("10001");
    expect(dataRows().length).toBeLessThan(100);
    await user.tab();
    await user.keyboard("{Control>}{End}{/Control}");
    await waitFor(() =>
      expect(document.activeElement?.closest("[role=row]")?.getAttribute("aria-rowindex")).toBe(
        "10001",
      ),
    );
  });
});

describe("DataGrid work per render at 10,000 rows (NFR-33)", () => {
  beforeEach(mockGridLayout);

  it("does no work proportional to the rows when a row opens, without bulk actions", async () => {
    const rows = items(10_000);
    function Opening() {
      const [openId, setOpenId] = useState<string | null>(null);
      return (
        <DataGrid
          label="Synthetic worklist"
          columns={COLUMNS}
          rows={rows}
          getRowId={getRowId}
          openRowId={openId}
          onRowAction={(item) => setOpenId(item.id)}
        />
      );
    }
    const { user } = renderWithProviders(<Opening />);
    const cell = screen.getByRole("gridcell", { name: "SYN-010000" });
    const lookups = vi.spyOn(Set.prototype, "has");
    await user.click(cell);
    expect(cell.closest("[role=row]")?.getAttribute("aria-current")).toBe("true");
    expect(lookups.mock.calls.length).toBeLessThan(1000);
  });
});
