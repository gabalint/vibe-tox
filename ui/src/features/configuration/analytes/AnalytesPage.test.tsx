import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ANALYTES, APPROVER, MANAGER, sessionWith } from "../testConfiguration";
import { AnalytesPage } from "./AnalytesPage";

const WITH_LOINC = {
  id: "00000000-0000-7000-8000-000000000b04",
  code: "SYN-OXY",
  name: "Synthetic oxycodone",
  loincCode: "00000-0",
  drugClassCode: "SYN-OPI",
};

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = MANAGER,
  path = "/configuration/analytes",
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_analytes: () => [...ANALYTES, WITH_LOINC],
    ...commands,
  });
  return renderRoutes([{ path: "/configuration/analytes", element: <AnalytesPage /> }], path);
}

const panel = (code: string) => screen.findByRole("region", { name: code });

describe("AnalytesPage (REQ-ANA-002)", () => {
  beforeEach(mockGridLayout);

  it("opens an analyte in the side panel, with codes as provided or Not provided", async () => {
    const { user, router, container } = renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "Analytes" })).toBeTruthy();
    await openRow(user, "SYN-CRE");
    const open = await panel("SYN-CRE");
    expect(within(open).getByText("Synthetic creatinine")).toBeTruthy();
    expect(within(open).getAllByText("Not provided")).toHaveLength(2);
    expect(router.state.location.search).toBe(`?open=${ANALYTES[1]?.id}`);
    await user.click(within(open).getByRole("button", { name: "Next" }));
    expect(within(await panel("SYN-OXY")).getByText("00000-0")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens with Enter, and Escape closes the panel with focus back on the row", async () => {
    const { user, router } = renderPage();
    await openRowWithEnter(user, "SYN-AMP");
    const open = await panel("SYN-AMP");
    within(open).getByRole("button", { name: "Close" }).focus();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "SYN-AMP" })).toBeNull());
    expect(router.state.location.search).toBe("");
    const row = screen.getByRole("gridcell", { name: "SYN-AMP" }).closest("[role=row]");
    await waitFor(() => expect(row?.contains(document.activeElement)).toBe(true));
  });

  it("finds analytes by LOINC", async () => {
    const { user } = renderPage();
    await user.type(
      await screen.findByRole("searchbox", { name: "Find analytes" }),
      "00000{Enter}",
    );
    expect(await screen.findByText("1 of 3 analytes")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "SYN-OXY" })).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "SYN-AMP" })).toBeNull();
  });

  it("adds an analyte, leaving codes not supplied out, then opens it", async () => {
    const created = { ...WITH_LOINC, id: "00000000-0000-7000-8000-000000000b05", code: "SYN-THC" };
    const create = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            {
              field: "code",
              code: "invalid_code",
              message: "Use up to 64 letters, digits, '.', '_' or '-'.",
            },
          ],
        }),
      )
      .mockReturnValue(created.id);
    let analytes = [...ANALYTES];
    const { user } = renderPage({
      list_analytes: () => analytes,
      create_analyte: (args) => {
        analytes = [...ANALYTES, created];
        return create(args);
      },
    });
    await user.click(await screen.findByRole("button", { name: "Add analyte" }));
    const dialog = await screen.findByRole("dialog", { name: "Add analyte" });
    const code = within(dialog).getByRole("textbox", { name: /^Code/ });
    await user.type(code, "SYN THC");
    await user.type(within(dialog).getByRole("textbox", { name: /^Name/ }), "Synthetic THC");
    await user.click(within(dialog).getByRole("button", { name: "Add analyte" }));
    expect(
      await within(dialog).findByText("Use up to 64 letters, digits, '.', '_' or '-'."),
    ).toBeTruthy();
    await user.clear(code);
    await user.type(code, "SYN-THC");
    await user.click(within(dialog).getByRole("button", { name: "Add analyte" }));
    expect(await screen.findByText("Analyte SYN-THC added")).toBeTruthy();
    expect(await panel("SYN-THC")).toBeTruthy();
    expect(create).toHaveBeenLastCalledWith({
      input: { code: "SYN-THC", name: "Synthetic THC", loincCode: null, drugClassCode: null },
    });
  });

  it("offers an approver no Add", async () => {
    renderPage({}, APPROVER);
    expect(await screen.findByRole("gridcell", { name: "SYN-AMP" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add analyte" })).toBeNull();
  });

  it("shows the empty, no-results and error states", async () => {
    const empty = renderPage({ list_analytes: () => [] });
    expect(screen.getByText("Loading analytes…")).toBeTruthy();
    expect(await screen.findByText("No analytes yet")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Add analyte" })).toHaveLength(2);
    empty.unmount();
    const found = renderPage({}, MANAGER, "/configuration/analytes?q=zzz");
    expect(await screen.findByText("No analytes match 'zzz'")).toBeTruthy();
    found.unmount();
    renderPage({
      list_analytes: () => Promise.reject(engineError("internal", "Synthetic failure.")),
    });
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The analytes could not be loaded",
    );
  });
});
