import { screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Dialog } from "@/components/Dialog";
import { MoreActions } from "@/components/RowActions";
import { useOpenRecord } from "@/lib/openRecord";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { renderRoutes } from "@/lib/test/router";
import { DetailLayout } from "./DetailLayout";
import { DetailPanel } from "./DetailPanel";
import { useWidePanel } from "./usePanelWidth";

interface Analyte {
  code: string;
  name: string;
}

const ANALYTES: Analyte[] = [
  { code: "SYN-B", name: "Synthetic analyte two" },
  { code: "SYN-A", name: "Synthetic analyte one" },
  { code: "SYN-C", name: "Synthetic analyte three" },
];

const COLUMNS: readonly DataGridColumn<Analyte>[] = [
  {
    id: "code",
    header: "Code",
    identifier: true,
    cell: (row) => row.code,
    sortValue: (row) => row.code,
  },
  { id: "name", header: "Name", cell: (row) => row.name },
];

/** Content that asks for a wide panel while it is shown, like a correction history. */
function WideContent() {
  const [shown, setShown] = useState(false);
  useWidePanel(shown);
  return (
    <Button onPress={() => setShown((value) => !value)}>{shown ? "Hide" : "Show"} table</Button>
  );
}

/** A directory screen as the standards compose it: list, open record in `?open=`, panel. */
function Directory({ wide = false }: { wide?: boolean }) {
  const record = useOpenRecord();
  const open = ANALYTES.find((row) => row.code === record.openId);
  return (
    <DetailLayout
      panel={
        open && (
          <DetailPanel
            identifier={open.code}
            pageHref={`/analytes/${open.code}`}
            actions={
              <>
                <Button variant="primary">Edit</Button>
                <MoreActions items={[{ id: "retire", label: "Retire", onAction: vi.fn() }]} />
              </>
            }
            onClose={record.close}
            onPrevious={record.previous}
            onNext={record.next}
          >
            <p>{open.name}</p>
            {wide && <WideContent />}
          </DetailPanel>
        )
      }
    >
      <DataGrid
        label="Synthetic analytes"
        columns={COLUMNS}
        rows={ANALYTES}
        getRowId={(row) => row.code}
        openRowId={record.openId}
        onOrderChange={record.followOrder}
        onRowAction={(row, order) => record.open(row.code, order)}
        fill
      />
    </DetailLayout>
  );
}

const renderDirectory = (path = "/analytes?q=syn", wide = false) =>
  renderRoutes([{ path: "/analytes", element: <Directory wide={wide} /> }], path);
const panel = (code: string) => screen.getByRole("region", { name: code });
const cell = (name: string) => screen.getByRole("gridcell", { name });

describe("DetailPanel", () => {
  beforeEach(mockGridLayout);

  it("opens from a row by click, keeps the other parameters, and Back closes it", async () => {
    const { user, router, container } = renderDirectory();
    await openRow(user, "SYN-A");
    expect(within(panel("SYN-A")).getByRole("heading", { level: 2, name: "SYN-A" })).toBeTruthy();
    expect(within(panel("SYN-A")).getByRole("link", { name: "Open page" })).toBeTruthy();
    expect(router.state.location.search).toBe("?q=syn&open=SYN-A");
    // Focus stays on the list, which stays usable beside the panel.
    expect(document.activeElement).toBe(cell("SYN-A"));
    await expectNoA11yViolations(container);
    await router.navigate(-1);
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-A" })).toBeNull());
    expect(router.state.location.search).toBe("?q=syn");
  });

  it("closes on Escape with focus back on the row, but Escape in its menu closes only the menu", async () => {
    const { user, router } = renderDirectory();
    await openRowWithEnter(user, "SYN-C");
    const more = within(panel("SYN-C")).getByRole("button", { name: "More" });
    await user.click(more);
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(more));
    expect(panel("SYN-C")).toBeTruthy();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-C" })).toBeNull());
    expect(router.state.location.search).toBe("?q=syn");
    await waitFor(() => expect(document.activeElement).toBe(cell("SYN-C")));
  });

  it("steps with Previous and Next in the list's displayed order, disabled at the ends", async () => {
    const { user, router } = renderDirectory();
    await user.click(screen.getByRole("columnheader", { name: "Code" }));
    await openRow(user, "SYN-B");
    const next = () => screen.getByRole("button", { name: "Next" });
    const previous = () => screen.getByRole("button", { name: "Previous" });
    await user.click(previous());
    expect(panel("SYN-A")).toBeTruthy();
    expect(previous().hasAttribute("disabled")).toBe(true);
    // Focus moved to the heading, rather than staying on a disabled button.
    expect(document.activeElement?.textContent).toBe("SYN-A");
    await user.click(next());
    await user.click(next());
    expect(panel("SYN-C")).toBeTruthy();
    expect(next().hasAttribute("disabled")).toBe(true);
    // Previous and Next replaced the entry: one Back closes the panel.
    await router.navigate(-1);
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-C" })).toBeNull());
  });

  it("follows the list's current order when it is sorted while the panel is open", async () => {
    const { user } = renderDirectory();
    // Opened in the engine's order B, A, C, where B is first.
    await openRow(user, "SYN-B");
    const code = screen.getByRole("columnheader", { name: "Code" });
    await user.click(code);
    await user.click(code);
    // Sorted descending, C, B, A: Previous from B is C.
    await user.click(screen.getByRole("button", { name: "Previous" }));
    expect(panel("SYN-C")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Previous" }).hasAttribute("disabled")).toBe(true);
  });

  it("stays open on Escape in a dialog opened from it, even one Escape does not close", async () => {
    function WithDialog() {
      const [open, setOpen] = useState(false);
      return (
        <MemoryRouter>
          <DetailPanel
            identifier="SYN-A"
            actions={<Button onPress={() => setOpen(true)}>Retire</Button>}
            onClose={onClose}
          >
            <Dialog title="Retire SYN-A" isOpen={open} onOpenChange={setOpen} isDismissable={false}>
              Synthetic confirmation
            </Dialog>
          </DetailPanel>
        </MemoryRouter>
      );
    }
    const onClose = vi.fn();
    const { user } = renderWithProviders(<WithDialog />);
    await user.click(screen.getByRole("button", { name: "Retire" }));
    await screen.findByRole("dialog", { name: "Retire SYN-A" });
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "Retire SYN-A" })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("steps through the list shown beside a record opened from a link", async () => {
    const { user } = renderDirectory("/analytes?open=SYN-A");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Previous" }).hasAttribute("disabled")).toBe(false),
    );
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(panel("SYN-C")).toBeTruthy();
  });
});

/** Reports `width` pixels for every observed element; jsdom has no layout. */
function stubResizeObserver(width: number) {
  class FixedWidthObserver {
    constructor(private readonly callback: (entries: object[]) => void) {}
    observe(target: Element) {
      this.callback([
        { target, contentRect: { width }, borderBoxSize: [{ inlineSize: width, blockSize: 320 }] },
      ]);
    }
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal("ResizeObserver", FixedWidthObserver);
}

describe("DetailPanel in a narrow window", () => {
  beforeEach(() => {
    mockGridLayout();
    stubResizeObserver(600);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("replaces the list and offers Back to list, which returns to the row", async () => {
    const { user, container } = renderDirectory();
    await openRow(user, "SYN-B");
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.queryByRole("button", { name: "Close" })).toBeNull();
    await waitFor(() => expect(document.activeElement?.textContent).toBe("SYN-B"));
    await expectNoA11yViolations(container);
    await user.click(screen.getByRole("button", { name: "Back to list" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-B" })).toBeNull());
    expect(screen.getByRole("grid")).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(cell("SYN-B")));
  });
});

describe("DetailPanel at a 1280px window", () => {
  beforeEach(mockGridLayout);
  afterEach(() => vi.unstubAllGlobals());

  // 1280px less the 224px navigation and the page's 2 × 24px padding (UI review finding 7).
  it("keeps the list beside the panel in the 1008px layout", async () => {
    stubResizeObserver(1008);
    const { user } = renderDirectory();
    await openRow(user, "SYN-B");
    expect(screen.getByRole("grid")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Back to list" })).toBeNull();
  });

  it("replaces the list once the list would drop below 38rem", async () => {
    stubResizeObserver(975);
    const { user } = renderDirectory();
    await openRow(user, "SYN-B");
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.getByRole("button", { name: "Back to list" })).toBeTruthy();
  });
});

describe("DetailPanel body", () => {
  beforeEach(() => {
    mockGridLayout();
    stubResizeObserver(2000);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("is a named region in the Tab order when its facts are taller than the panel", async () => {
    const { user, container } = renderDirectory();
    await openRow(user, "SYN-A");
    const body = within(panel("SYN-A")).getByRole("region", { name: "Details of SYN-A" });
    expect(body.tabIndex).toBe(0);
    await expectNoA11yViolations(container);
  });

  it("adds no tab stop when its facts fit", async () => {
    vi.spyOn(Element.prototype, "scrollHeight", "get").mockReturnValue(320);
    const { user } = renderDirectory();
    await openRow(user, "SYN-A");
    expect(within(panel("SYN-A")).queryByRole("region")).toBeNull();
    expect(
      within(panel("SYN-A")).getByText("Synthetic analyte one").closest("[tabindex]"),
    ).toBeNull();
  });
});

describe("DetailPanel width", () => {
  const splitter = () => screen.getByRole("separator", { name: "Width of SYN-A" });
  const width = () => Number(splitter().getAttribute("aria-valuenow"));

  // A 2000px layout: the panel goes from 22rem (352px) to 64rem (1024px), 34rem (544px) by default.
  beforeEach(() => {
    localStorage.clear();
    mockGridLayout();
    stubResizeObserver(2000);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("resizes from its left edge with the keyboard and remembers the width for the page", async () => {
    const { user, unmount } = renderDirectory();
    await openRow(user, "SYN-A");
    expect(width()).toBe(544);
    splitter().focus();
    await user.keyboard("{ArrowLeft}");
    expect(width()).toBe(576);
    await user.keyboard("{End}");
    expect(width()).toBe(1024);
    await user.keyboard("{Home}{ArrowLeft}");
    expect(width()).toBe(384);
    unmount();

    renderDirectory("/analytes?open=SYN-A");
    await waitFor(() => expect(width()).toBe(384));
  });

  it("keeps the list its 38rem beside the panel", async () => {
    stubResizeObserver(1008);
    const { user } = renderDirectory();
    await openRow(user, "SYN-A");
    splitter().focus();
    await user.keyboard("{End}");
    // 1008px less the 38rem list and the 1rem gap.
    expect(width()).toBe(384);
  });

  it("widens while its content asks, then returns to the width chosen before", async () => {
    const { user } = renderDirectory("/analytes?open=SYN-A", true);
    splitter().focus();
    await user.keyboard("{ArrowRight}");
    expect(width()).toBe(512);
    await user.click(screen.getByRole("button", { name: "Show table" }));
    expect(width()).toBe(1024);
    splitter().focus();
    await user.keyboard("{ArrowRight}");
    expect(width()).toBe(992);
    await user.click(screen.getByRole("button", { name: "Hide table" }));
    expect(width()).toBe(512);
  });
});
