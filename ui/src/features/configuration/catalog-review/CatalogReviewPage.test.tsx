import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { PREVIEW_ID, preview, REVIEWED, renderCatalogReview } from "./testData";

type Handler = (args: unknown) => unknown;
const BOTH = ["manage_configuration", "approve_configuration"];

function renderPage(capabilities: readonly string[], commands: Record<string, Handler> = {}) {
  return renderCatalogReview(capabilities, {
    list_catalog_previews: () => [preview(), REVIEWED],
    view_catalog_preview_pdf: () => null,
    ...commands,
  });
}

describe("Catalog review (REQ-OPS-015)", () => {
  beforeEach(() => mockGridLayout());

  it("reviews laboratory-entered catalog drafts without reading any starter data", async () => {
    const starter = vi.fn();
    const { container } = renderPage(BOTH, {
      standard_catalog_tree: starter,
      standard_catalog_status: starter,
      demo_status: starter,
    });
    await screen.findByRole("gridcell", { name: PREVIEW_ID });
    expect(screen.getByRole("heading", { level: 1, name: "Catalog review" })).toBeTruthy();
    expect(screen.getByText(/regardless of how they were entered/)).toBeTruthy();
    expect(starter).not.toHaveBeenCalled();
    await expectNoA11yViolations(container);
  });

  it("filters reports by person, keeps the find in the URL and clears no-results", async () => {
    const { user, router } = renderPage(BOTH);
    await screen.findByRole("gridcell", { name: PREVIEW_ID });
    const find = screen.getByRole("searchbox", { name: "Find catalog previews" });
    await user.type(find, "Robin{Enter}");
    await waitFor(() => expect(screen.queryByRole("gridcell", { name: PREVIEW_ID })).toBeNull());
    expect(screen.getByRole("gridcell", { name: REVIEWED.id })).toBeTruthy();
    expect(router.state.location.search).toBe("?q=Robin");
    await user.clear(find);
    await user.type(find, "missing{Enter}");
    expect(await screen.findByText("No catalog previews match 'missing'")).toBeTruthy();
    const clear = screen.getAllByRole("button", { name: "Clear search" })[0];
    if (!clear) throw new Error("No clear-search action");
    await user.click(clear);
    expect(await screen.findByRole("gridcell", { name: PREVIEW_ID })).toBeTruthy();
  });

  it("shows a report-list failure with its support reference and retries", async () => {
    const list = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "The catalog reports could not be read."))
      .mockResolvedValueOnce([preview()]);
    const { user } = renderPage(BOTH, { list_catalog_previews: list });
    expect(await screen.findByText("The catalog reports could not be read.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("gridcell", { name: PREVIEW_ID })).toBeTruthy();
  });

  it("generates a Catalog Preview, lists it and opens it with its PDF", async () => {
    const newId = "00000000-0000-7000-8000-00000000d002";
    let reports = [REVIEWED];
    const generate = vi.fn(() => {
      reports = [preview({ id: newId }), REVIEWED];
      return newId;
    });
    const view = vi.fn(() => null);
    const { user, router } = renderPage(BOTH, {
      list_catalog_previews: () => reports,
      generate_catalog_preview: generate,
      view_catalog_preview_pdf: view,
    });
    await screen.findByRole("gridcell", { name: REVIEWED.id });
    await user.click(screen.getByRole("button", { name: "Generate Catalog Preview" }));
    const panel = await screen.findByRole("region", { name: newId });
    expect(router.state.location.search).toBe(`?open=${newId}`);
    expect(await screen.findByText("Catalog Preview generated")).toBeTruthy();
    expect(within(panel).getByText("Drafts listed").nextSibling?.textContent).toBe("5");
    expect(within(panel).getByText("cd34")).toBeTruthy();
    expect(view).toHaveBeenCalledWith({ input: { previewId: newId } });
    const rows = screen.getAllByRole("row").slice(1);
    expect(rows.map((row) => within(row).getAllByRole("gridcell")[0]?.textContent)).toEqual([
      newId,
      REVIEWED.id,
    ]);
  });

  it("shows an empty list and a refused generation", async () => {
    const { user } = renderPage(BOTH, {
      list_catalog_previews: () => [],
      generate_catalog_preview: () =>
        Promise.reject(
          engineError(
            "conflict",
            "The catalog changed while the report was generated. Generate it again.",
          ),
        ),
    });
    expect(await screen.findByText("No Catalog Preview yet")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Generate Catalog Preview" }));
    expect(await screen.findByText("Not generated")).toBeTruthy();
    expect(screen.getAllByText(/Generate it again/).length).toBeGreaterThan(0);
  });

  it("shows a clicked report's PDF in a report window, and again from its panel", async () => {
    const view = vi.fn(() => null);
    const { user } = renderPage(BOTH, { view_catalog_preview_pdf: view });
    await openRow(user, PREVIEW_ID);
    const panel = within(await screen.findByRole("region", { name: PREVIEW_ID }));
    await waitFor(() => expect(view).toHaveBeenCalledWith({ input: { previewId: PREVIEW_ID } }));
    await user.click(panel.getByRole("button", { name: "View report" }));
    await waitFor(() => expect(view).toHaveBeenCalledTimes(2));
  });

  it("explains a report that could not be shown", async () => {
    const { user } = renderPage(BOTH, {
      view_catalog_preview_pdf: () =>
        Promise.reject(engineError("unavailable", "The stored report file is damaged.")),
    });
    await openRow(user, PREVIEW_ID);
    expect(await screen.findByText("Not shown")).toBeTruthy();
    expect(screen.getByText(/The stored report file is damaged/)).toBeTruthy();
  });

  it("saves a report's PDF to Downloads, naming the file", async () => {
    const save = vi.fn(() => ({ fileName: "catalog-preview-d001.pdf", folder: "downloads" }));
    const { user } = renderPage(BOTH, { export_catalog_preview_pdf: save });
    await openRow(user, PREVIEW_ID);
    const panel = within(await screen.findByRole("region", { name: PREVIEW_ID }));
    await user.click(panel.getByRole("button", { name: "Save PDF" }));
    expect(
      await screen.findByText("Saved catalog-preview-d001.pdf to your Downloads folder."),
    ).toBeTruthy();
    expect(save).toHaveBeenCalledWith({ input: { previewId: PREVIEW_ID } });
  });

  it("explains a PDF that could not be saved", async () => {
    const { user } = renderPage(BOTH, {
      export_catalog_preview_pdf: () =>
        Promise.reject(engineError("unavailable", "No Downloads folder.")),
    });
    const menu = within(await openRowActions(user, PREVIEW_ID));
    await user.click(menu.getByRole("menuitem", { name: "Save PDF" }));
    expect(await screen.findByText("Not saved")).toBeTruthy();
    expect(screen.getByText("The PDF could not be saved to your Downloads folder.")).toBeTruthy();
  });

  it("offers configuration managers View report and Save PDF on a report", async () => {
    const { user } = renderPage(["manage_configuration"]);
    const menu = within(await openRowActions(user, PREVIEW_ID));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "View report",
      "Save PDF",
    ]);
  });

  it("offers approvers Approve reviewed catalog on unreviewed reports only", async () => {
    const { user } = renderPage(["approve_configuration"]);
    let menu = within(await openRowActions(user, PREVIEW_ID));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "View report",
      "Save PDF",
      "Approve reviewed catalog",
    ]);
    await user.keyboard("{Escape}");
    menu = within(await openRowActions(user, REVIEWED.id));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "View report",
      "Save PDF",
    ]);
  });

  it("opens a report with Enter, and Escape closes it with focus back on the row", async () => {
    const { user } = renderPage(BOTH);
    await screen.findByRole("gridcell", { name: PREVIEW_ID });
    await openRowWithEnter(user, PREVIEW_ID);
    const panel = await screen.findByRole("region", { name: PREVIEW_ID });
    expect(within(panel).getByText("Blank cutoffs to confirm").nextSibling?.textContent).toBe("2");
    await user.click(within(panel).getByRole("heading", { name: PREVIEW_ID }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: PREVIEW_ID })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: PREVIEW_ID })),
    );
  });
});
