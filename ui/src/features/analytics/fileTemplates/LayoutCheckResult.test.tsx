import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { LayoutCheckResult } from "./LayoutCheckResult";
import { LONG_READING, MORPHINE, WIDE_READING } from "./testTemplates";

const analyteName = (id: string) => (id === MORPHINE.id ? "Synthetic morphine (SYN-MOR)" : id);

describe("LayoutCheckResult (result files brief §7)", () => {
  beforeEach(mockGridLayout);

  it("shows a wide file's own header text beside the mapped analyte", async () => {
    const check = {
      ...WIDE_READING,
      rows: WIDE_READING.rows.map((row) => ({
        ...row,
        headerText: row.analyteName,
        analyteId: row.analyteName === "Morphine Results" ? MORPHINE.id : null,
      })),
    };
    const { container } = renderWithProviders(
      <LayoutCheckResult check={check} analyteName={analyteName} />,
    );
    const grid = screen.getByRole("grid", { name: "Rows read from the file" });
    expect(within(grid).getByRole("columnheader", { name: /File's header text/ })).toBeTruthy();
    expect(within(grid).getAllByRole("gridcell", { name: "Morphine Results" })).toHaveLength(2);
    expect(
      within(grid).getByRole("gridcell", { name: "Synthetic morphine (SYN-MOR)" }),
    ).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("has no header text column for a long file, whose rows carry the name", () => {
    renderWithProviders(<LayoutCheckResult check={LONG_READING} analyteName={analyteName} />);
    const grid = screen.getByRole("grid", { name: "Rows read from the file" });
    expect(within(grid).queryByRole("columnheader", { name: /File's header text/ })).toBeNull();
  });
});
