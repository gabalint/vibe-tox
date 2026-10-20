import { act, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "@/components/Button";
import { useFindParam } from "./findParam";
import { useOpenRecord } from "./openRecord";
import { renderRoutes } from "./test/router";

const ORDER = ["SYN-1", "SYN-2", "SYN-3"];

function Harness() {
  const record = useOpenRecord();
  const [, setQuery] = useFindParam();
  return (
    <>
      <p>Open: {record.openId ?? "none"}</p>
      <Button onPress={() => setQuery("oxy")}>Find oxy</Button>
      <Button onPress={() => record.open("SYN-2", ORDER)}>Open second</Button>
      <Button onPress={() => record.open("SYN-3")}>Open third</Button>
      <Button onPress={record.close}>Close</Button>
      <Button isDisabled={!record.previous} onPress={record.previous}>
        Previous
      </Button>
      <Button isDisabled={!record.next} onPress={record.next}>
        Next
      </Button>
    </>
  );
}

const renderHarness = (path: string) =>
  renderRoutes([{ path: "/list", element: <Harness /> }], path);
const press = (name: string) => screen.getByRole("button", { name });

describe("useOpenRecord", () => {
  it("opens into the URL with a new history entry and keeps the other parameters", async () => {
    const { user, router } = renderHarness("/list?q=syn");
    await user.click(press("Open second"));
    expect(router.state.location.search).toBe("?q=syn&open=SYN-2");
    expect(router.state.historyAction).toBe("PUSH");
    // Opening another record replaces the entry, so one Back closes the panel.
    await user.click(press("Open third"));
    expect(router.state.historyAction).toBe("REPLACE");
    await router.navigate(-1);
    await waitFor(() => expect(screen.getByText("Open: none")).toBeTruthy());
  });

  it("steps through the list's order by replacing the entry, and closes by going back", async () => {
    const { user, router } = renderHarness("/list?q=syn");
    await user.click(press("Open second"));
    await user.click(press("Next"));
    expect(screen.getByText("Open: SYN-3")).toBeTruthy();
    expect(router.state.historyAction).toBe("REPLACE");
    expect(press("Next").hasAttribute("disabled")).toBe(true);
    await user.click(press("Previous"));
    await user.click(press("Previous"));
    expect(screen.getByText("Open: SYN-1")).toBeTruthy();
    expect(press("Previous").hasAttribute("disabled")).toBe(true);
    await user.click(press("Close"));
    await waitFor(() => expect(router.state.location.search).toBe("?q=syn"));
    expect(router.state.historyAction).toBe("POP");
  });

  it("keeps find text changed while the panel was open, removing only the parameter", async () => {
    const { user, router } = renderHarness("/list?q=syn");
    await user.click(press("Open second"));
    await user.click(press("Find oxy"));
    expect(router.state.location.search).toBe("?q=oxy&open=SYN-2");
    await user.click(press("Close"));
    expect(router.state.location.search).toBe("?q=oxy");
    expect(router.state.historyAction).toBe("REPLACE");
    expect(router.state.location.state).toBeNull();
  });

  it("keeps a record page's navigation state through open, Next and close", async () => {
    const pageState = { recordSequence: { list: { label: "Runs", href: "/runs" }, ids: ["R1"] } };
    const { user, router } = renderHarness("/list");
    await act(() => router.navigate("/list?tab=positions", { state: pageState }));
    await user.click(press("Open second"));
    expect(router.state.location.state).toMatchObject(pageState);
    await user.click(press("Next"));
    expect(router.state.location.state).toMatchObject(pageState);
    await user.click(press("Close"));
    await waitFor(() => expect(router.state.location.search).toBe("?tab=positions"));
    expect(router.state.location.state).toEqual(pageState);

    // Closed in place (the parameters changed meanwhile), the page's state stays too.
    await user.click(press("Open second"));
    await user.click(press("Find oxy"));
    await user.click(press("Close"));
    expect(router.state.location.search).toBe("?tab=positions&q=oxy");
    expect(router.state.location.state).toEqual(pageState);
  });

  it("closes a record opened from a link by removing the parameter, with no order to step", async () => {
    const { user, router } = renderHarness("/list?open=SYN-2&q=syn");
    expect(screen.getByText("Open: SYN-2")).toBeTruthy();
    expect(press("Previous").hasAttribute("disabled")).toBe(true);
    expect(press("Next").hasAttribute("disabled")).toBe(true);
    await user.click(press("Close"));
    expect(router.state.location.search).toBe("?q=syn");
    expect(router.state.historyAction).toBe("REPLACE");
  });
});
