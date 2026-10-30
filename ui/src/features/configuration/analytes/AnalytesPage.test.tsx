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
  active: true,
  revision: 1,
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

const RETIRED = {
  ...WITH_LOINC,
  id: "00000000-0000-7000-8000-000000000b06",
  code: "SYN-OLD",
  name: "Synthetic retired",
  active: false,
  revision: 2,
};

const UNUSED = { codeInUse: false, methods: [] };
const USED = {
  codeInUse: true,
  methods: [
    { methodCode: "SYN-IA", methodName: "Synthetic immunoassay", version: 1, status: "approved" },
  ],
};

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
    expect(await screen.findByText("1 of 3 analytes · retired excluded")).toBeTruthy();
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

  it("corrects an unused analyte's code and name with a reason, then shows its history", async () => {
    const save = vi.fn().mockReturnValue({ ...ANALYTES[0], code: "SYN-AMPH", revision: 2 });
    const { user } = renderPage({
      analyte_usage: () => UNUSED,
      update_analyte: save,
      analyte_history: () => [
        {
          revision: 1,
          code: "SYN-AMP",
          name: "Synthetic amphetamine",
          loincCode: null,
          drugClassCode: "SYN-STIM",
          active: true,
          recordedAt: "2026-10-29T08:00:00Z",
          recordedBy: null,
          reason: null,
          origin: "legacy_baseline",
        },
      ],
    });
    await openRow(user, "SYN-AMP");
    const open = await panel("SYN-AMP");
    expect(within(open).getByText("Active")).toBeTruthy();
    await user.click(within(open).getByRole("button", { name: "Correct analyte" }));
    const dialog = await screen.findByRole("dialog", { name: "Correct analyte" });
    const code = within(dialog).getByRole("textbox", { name: /^Code/ });
    await user.clear(code);
    await user.type(code, "SYN-AMPH");
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Supplier sheet");
    await user.click(within(dialog).getByRole("button", { name: "Save correction" }));
    expect(await screen.findByText("Analyte correction saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        analyteId: ANALYTES[0]?.id,
        expectedRevision: 1,
        code: "SYN-AMPH",
        name: "Synthetic amphetamine",
        loincCode: null,
        drugClassCode: "SYN-STIM",
        active: true,
        reason: "Supplier sheet",
      },
    });
    await user.click(within(open).getByRole("button", { name: "Show correction history" }));
    expect(await within(open).findByText("Unknown — legacy baseline")).toBeTruthy();
  });

  it("keeps a used code permanent, names its methods and sends the current code", async () => {
    const save = vi.fn().mockReturnValue({ ...ANALYTES[0], revision: 2 });
    const { user } = renderPage({ analyte_usage: () => USED, update_analyte: save });
    await openRow(user, "SYN-AMP");
    await user.click(
      within(await panel("SYN-AMP")).getByRole("button", { name: "Correct analyte" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Correct analyte" });
    const code = within(dialog).getByRole("textbox", { name: /^Code/ }) as HTMLInputElement;
    expect(code.disabled).toBe(true);
    expect(code.value).toBe("SYN-AMP");
    const notes = within(within(dialog).getByRole("status")).getAllByRole("listitem");
    expect(notes[0]?.textContent).toMatch(/^Code SYN-AMP is permanent/);
    expect(notes[1]?.textContent).toMatch(/^Used by SYN-IA version 1 \(approved\)/);
    const name = within(dialog).getByRole("textbox", { name: /^Name/ });
    await user.clear(name);
    await user.type(name, "Synthetic amphetamine corrected");
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Typo");
    await user.click(within(dialog).getByRole("button", { name: "Save correction" }));
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(save.mock.calls[0]?.[0]).toMatchObject({
      input: { code: "SYN-AMP", name: "Synthetic amphetamine corrected" },
    });
  });

  it("hides retired analytes until Show retired, and offers an approver no correction", async () => {
    const { user } = renderPage({ list_analytes: () => [...ANALYTES, RETIRED] }, APPROVER);
    expect(await screen.findByText("2 analytes · retired excluded")).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "SYN-OLD" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.click(screen.getByRole("checkbox", { name: "Show retired" }));
    await openRow(user, "SYN-OLD");
    const open = await panel("SYN-OLD");
    expect(within(open).getAllByText("Retired").length).toBeGreaterThan(0);
    expect(within(open).queryByRole("button", { name: "Correct analyte" })).toBeNull();
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
