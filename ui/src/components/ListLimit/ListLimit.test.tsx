import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { renderWithProviders } from "@/lib/test/render";
import { ListCount, ListLimit, withinLimit } from "./ListLimit";

const REST = "Narrow the filter to see the rest; sorting a column orders only the rows shown.";

describe("ListLimit", () => {
  it("uses the engine's count when the answer has one", async () => {
    const { container } = renderWithProviders(
      <ListLimit shown={200} end="oldest" total={{ count: 340, capped: false }} />,
    );
    expect(screen.getByText(`Showing the oldest 200 of 340. ${REST}`)).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("says when the engine stopped counting", () => {
    renderWithProviders(
      <ListLimit shown={200} end="newest" total={{ count: 500, capped: true }} />,
    );
    expect(screen.getByText(`Showing the newest 200 of 500 or more. ${REST}`)).toBeTruthy();
  });

  it("without a count, says more exist when the extra row came back", () => {
    const answer = Array.from({ length: 201 }, (_, index) => `SYN-${index}`);
    const { rows, hasMore } = withinLimit(answer, 200);
    expect(rows).toHaveLength(200);
    expect(rows.at(-1)).toBe("SYN-199");
    renderWithProviders(<ListLimit shown={rows.length} end="first" hasMore={hasMore} />);
    expect(screen.getByText(`Showing the first 200. ${REST}`)).toBeTruthy();
  });

  it("shows nothing when every row is shown", () => {
    const { rows, hasMore } = withinLimit(["SYN-1", "SYN-2"], 200);
    expect(hasMore).toBe(false);
    const { container, rerender } = renderWithProviders(
      <ListLimit shown={rows.length} end="oldest" hasMore={hasMore} />,
    );
    expect(container.textContent).toBe("");
    rerender(<ListLimit shown={2} end="oldest" total={{ count: 2, capped: false }} />);
    expect(container.textContent).toBe("");
  });

  it("keeps subset ordering separate from the toolbar count", () => {
    const { container } = renderWithProviders(
      <ListLimit variant="summary" shown={200} end="oldest" hasMore />,
    );
    expect(container.textContent).toBe(
      "Showing the oldest results; sorting a column orders only the rows shown.",
    );
  });
});

describe("ListCount", () => {
  it.each([
    { shown: 1, expected: "1 record shown" },
    { shown: 200, hasMore: true, expected: "200 records shown · more available" },
    { shown: 0, total: { count: 0, capped: false }, expected: "0 records" },
    { shown: 1, total: { count: 1, capped: false }, expected: "1 record" },
    { shown: 200, total: { count: 340, capped: false }, expected: "200 of 340 records" },
    {
      shown: 200,
      total: { count: 1000, capped: true },
      expected: "200 shown · 1,000 or more records",
    },
  ])("describes only the count evidence: $expected", ({ expected, ...props }) => {
    const { container } = renderWithProviders(<ListCount {...props} noun="records" one="record" />);
    expect(container.textContent).toBe(expected);
  });
});
