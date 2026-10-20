import { act, screen, waitFor } from "@testing-library/react";
import { StrictMode, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import { QueueScanControls } from "./QueueScanControls";
import { engineError, mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";
import { useBoundedQueue } from "./useBoundedQueue";

interface Row {
  id: string;
}
interface Page {
  items: Row[];
  next: string | null;
  scanned: number;
}
const page = (items: Row[], next: string | null = null): Page => ({ items, next, scanned: 500 });
function Probe({ filter = "stale" }: { filter?: string }) {
  const query = useBoundedQueue<Row, string>("list_review_queue", {
    key: { filter },
    pageArgs: (after) => ({ input: { after } }),
    rowId: (row) => row.id,
  });
  return (
    <>
      <Button onPress={() => void query.restartScan()}>Refresh</Button>
      <QueryContent
        query={query}
        what="review queue"
        empty={{
          when: () => query.rows.length === 0 && !query.hasNextPage,
          title: "Nothing waiting",
          description: "The complete check found no matches.",
        }}
      >
        {() => <output>{query.rows.map((row) => row.id).join(",")}</output>}
      </QueryContent>
      {query.data && <QueueScanControls query={query} review />}
    </>
  );
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("bounded queue scans", () => {
  it("continues checked-empty pages without declaring empty and pauses after the active request", async () => {
    const pending = deferred<Page>();
    const read = vi
      .fn()
      .mockReturnValueOnce(page([], "checked-500"))
      .mockReturnValueOnce(pending.promise)
      .mockReturnValueOnce(page([{ id: "match" }]));
    mockCommands({ list_review_queue: read });
    const { user } = renderWithProviders(
      <StrictMode>
        <Probe />
      </StrictMode>,
    );
    await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
    expect(screen.queryByText("Nothing waiting")).toBeNull();
    expect(screen.getByText(/500 candidate specimens checked · 0 matches loaded/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Pause checking" }));
    await act(async () => pending.resolve(page([], "checked-1000")));
    expect(await screen.findByText(/Check paused/)).toBeTruthy();
    expect(read).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Resume checking" }));
    expect(await screen.findByText("match")).toBeTruthy();
    expect(read).toHaveBeenLastCalledWith({ input: { after: "checked-1000" } });
    expect(screen.getByText(/Check complete/)).toBeTruthy();
  });

  it("retains matches on failure and retries the exact checked cursor, then refreshes at null", async () => {
    const read = vi
      .fn()
      .mockReturnValueOnce(page([{ id: "kept" }], "checked-not-a-match"))
      .mockRejectedValueOnce(engineError("conflict", "Temporary read failure"))
      .mockReturnValueOnce(page([{ id: "later" }]))
      .mockReturnValueOnce(page([{ id: "fresh" }]));
    mockCommands({ list_review_queue: read });
    const { user } = renderWithProviders(<Probe />);
    expect(await screen.findByText("Temporary read failure")).toBeTruthy();
    expect(screen.getByText("kept")).toBeTruthy();
    expect(read).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("kept,later")).toBeTruthy();
    expect(read).toHaveBeenLastCalledWith({ input: { after: "checked-not-a-match" } });
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(await screen.findByText("fresh")).toBeTruthy();
    expect(read).toHaveBeenLastCalledWith({ input: { after: null } });
    expect(read).toHaveBeenCalledTimes(4);
  });

  it("stops scheduling when the route leaves while a request is pending", async () => {
    const pending = deferred<Page>();
    const read = vi
      .fn()
      .mockReturnValueOnce(page([], "first"))
      .mockReturnValueOnce(pending.promise);
    mockCommands({ list_review_queue: read });
    const { unmount } = renderWithProviders(<Probe />);
    await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
    unmount();
    await act(async () => pending.resolve(page([], "second")));
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("starts a new null-cursor scan when returning to an already completed filter", async () => {
    const read = vi
      .fn()
      .mockReturnValueOnce(page([{ id: "old-stale" }]))
      .mockReturnValueOnce(page([{ id: "blocked" }]))
      .mockReturnValueOnce(page([{ id: "new-stale" }]));
    mockCommands({ list_review_queue: read });
    function FilterProbe() {
      const [filter, setFilter] = useState("stale");
      return (
        <>
          <Button onPress={() => setFilter(filter === "stale" ? "blocked" : "stale")}>
            Change filter
          </Button>
          <Probe filter={filter} />
        </>
      );
    }
    const { user } = renderWithProviders(<FilterProbe />);
    expect(await screen.findByText("old-stale")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Change filter" }));
    expect(await screen.findByText("blocked")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Change filter" }));
    expect(await screen.findByText("new-stale")).toBeTruthy();
    expect(read.mock.calls.map(([args]) => args)).toEqual([
      { input: { after: null } },
      { input: { after: null } },
      { input: { after: null } },
    ]);
  });

  it("starts at null when the screen remounts with the same query client", async () => {
    const read = vi
      .fn()
      .mockReturnValueOnce(page([{ id: "before" }]))
      .mockReturnValueOnce(page([{ id: "after" }]));
    mockCommands({ list_review_queue: read });
    function RouteProbe() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <Button onPress={() => setOpen(!open)}>Toggle route</Button>
          {open && <Probe />}
        </>
      );
    }
    const { user } = renderWithProviders(<RouteProbe />);
    expect(await screen.findByText("before")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Toggle route" }));
    await user.click(screen.getByRole("button", { name: "Toggle route" }));
    expect(await screen.findByText("after")).toBeTruthy();
    expect(read).toHaveBeenLastCalledWith({ input: { after: null } });
    expect(read).toHaveBeenCalledTimes(2);
  });

  it("keeps a double-click retry single-flight", async () => {
    const pending = deferred<Page>();
    const read = vi
      .fn()
      .mockReturnValueOnce(page([{ id: "kept" }], "cursor"))
      .mockRejectedValueOnce(engineError("conflict", "Retry this read"))
      .mockReturnValueOnce(pending.promise);
    mockCommands({ list_review_queue: read });
    const { user } = renderWithProviders(<Probe />);
    const retry = await screen.findByRole("button", { name: "Try again" });
    await user.dblClick(retry);
    expect(read).toHaveBeenCalledTimes(3);
    await act(async () => pending.resolve(page([{ id: "recovered" }])));
    expect(await screen.findByText("kept,recovered")).toBeTruthy();
    expect(read).toHaveBeenCalledTimes(3);
  });

  it("does not let an older refresh revive its scan after rapid filter changes", async () => {
    const pending = deferred<Page>();
    const read = vi
      .fn()
      .mockReturnValueOnce(page([{ id: "initial" }]))
      .mockReturnValueOnce(pending.promise)
      .mockReturnValueOnce(page([{ id: "other-filter" }]))
      .mockReturnValueOnce(page([{ id: "fresh-return" }]));
    mockCommands({ list_review_queue: read });
    function Filters() {
      const [filter, setFilter] = useState("stale");
      return (
        <>
          <Button onPress={() => setFilter(filter === "stale" ? "blocked" : "stale")}>
            Switch filter
          </Button>
          <Probe filter={filter} />
        </>
      );
    }
    const { user } = renderWithProviders(<Filters />);
    expect(await screen.findByText("initial")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(read).toHaveBeenCalledTimes(2));
    await user.click(screen.getByRole("button", { name: "Switch filter" }));
    expect(await screen.findByText("other-filter")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Switch filter" }));
    expect(await screen.findByText("fresh-return")).toBeTruthy();
    await act(async () => pending.resolve(page([{ id: "obsolete" }], "old-cursor")));
    expect(screen.getByText("fresh-return")).toBeTruthy();
    expect(screen.queryByText("obsolete")).toBeNull();
    expect(read).toHaveBeenCalledTimes(4);
    expect(read.mock.calls.every(([args]) => args.input.after === null)).toBe(true);
  });

  it("only declares an empty queue once the engine returns no continuation", async () => {
    const read = vi.fn().mockReturnValueOnce(page([], "first")).mockReturnValueOnce(page([]));
    mockCommands({ list_review_queue: read });
    renderWithProviders(<Probe />);
    expect(await screen.findByText("Nothing waiting")).toBeTruthy();
    expect(read).toHaveBeenCalledTimes(2);
    expect(screen.getByText(/1,000 candidate specimens checked/)).toBeTruthy();
  });
});
