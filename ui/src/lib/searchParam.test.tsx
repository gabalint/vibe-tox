import { act, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "@/components/Button";
import { useSearchParam } from "./searchParam";
import { renderRoutes } from "./test/router";

function Filter() {
  const [status, setStatus] = useSearchParam("status");
  return (
    <>
      <p>Status: {status ?? "any"}</p>
      <Button onPress={() => setStatus("open")}>Open only</Button>
      <Button onPress={() => setStatus(null)}>Clear filters</Button>
    </>
  );
}

const renderFilter = () => renderRoutes([{ path: "/runs", element: <Filter /> }], "/runs");
const press = (name: string) => screen.getByRole("button", { name });

describe("useSearchParam", () => {
  it("sets and clears one parameter in place, keeping the others and the navigation state", async () => {
    const { user, router } = renderFilter();
    const state = { recordSequence: { list: { label: "Runs", href: "/runs" }, ids: [] } };
    await act(() => router.navigate("/runs?q=syn&open=R1", { state }));
    await user.click(press("Open only"));
    expect(screen.getByText("Status: open")).toBeTruthy();
    expect(router.state.location.search).toBe("?q=syn&open=R1&status=open");
    expect(router.state.historyAction).toBe("REPLACE");
    expect(router.state.location.state).toEqual(state);
    await user.click(press("Clear filters"));
    expect(router.state.location.search).toBe("?q=syn&open=R1");
    expect(router.state.location.state).toEqual(state);
  });

  it("reads the parameter from the address", async () => {
    const { router } = renderFilter();
    await act(() => router.navigate("/runs?status=closed"));
    expect(screen.getByText("Status: closed")).toBeTruthy();
  });
});
