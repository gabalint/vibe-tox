import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FindField, findRows, NoResults } from "@/components/FindField";
import { useFindParam } from "./findParam";
import { renderRoutes } from "./test/router";

const ANALYTES = [
  { code: "SYN-OXY", name: "Synthetic analyte one" },
  { code: "SYN-MOR", name: "Synthetic analyte two" },
  { code: "SYN-FEN", name: "Synthetic analyte three" },
];

/** A directory as screens compose it: find field, count, rows, and the no-results state. */
function AnalytesDirectory() {
  const [query, setQuery] = useFindParam();
  const rows = findRows(ANALYTES, query, (row) => [row.code, row.name]);
  return (
    <>
      <FindField
        label="Find analytes"
        value={query}
        onChange={setQuery}
        count={{ shown: rows.length, total: ANALYTES.length, noun: "analytes", one: "analyte" }}
      />
      {rows.length === 0 ? (
        <NoResults noun="analytes" query={query} onClear={() => setQuery("")} />
      ) : (
        <ul>
          {rows.map((row) => (
            <li key={row.code}>{row.code}</li>
          ))}
        </ul>
      )}
    </>
  );
}

const renderDirectory = (path: string) =>
  renderRoutes([{ path: "/analytes", element: <AnalytesDirectory /> }], path);
const field = () => screen.getByRole("searchbox", { name: "Find analytes" });

describe("useFindParam", () => {
  it("keeps the find text in ?q= by replacing the entry, with the other parameters", async () => {
    const { user, router } = renderDirectory("/analytes?retired=show");
    await user.type(field(), "mor");
    await waitFor(() => expect(router.state.location.search).toBe("?retired=show&q=mor"));
    expect(router.state.historyAction).toBe("REPLACE");
    // Router state can publish before React commits the navigation's rendered rows.
    await waitFor(() => {
      expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(["SYN-MOR"]);
      expect(screen.getByRole("status").textContent).toBe("1 of 3 analytes");
    });
  });

  it("starts from the find text in the address", () => {
    renderDirectory("/analytes?q=fen");
    expect(field()).toHaveProperty("value", "fen");
    expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(["SYN-FEN"]);
  });

  it("shows no results distinctly, and Clear search empties the field and the address", async () => {
    const { user, router } = renderDirectory("/analytes?q=zzz");
    expect(screen.getByText("No analytes match 'zzz'")).toBeTruthy();
    // The field's own clear button has the same name; this is the one in the no-results state.
    await user.click(screen.getByText("Clear search"));
    expect(router.state.location.search).toBe("");
    expect(field()).toHaveProperty("value", "");
    expect(screen.getByRole("status").textContent).toBe("3 analytes");
  });
});
