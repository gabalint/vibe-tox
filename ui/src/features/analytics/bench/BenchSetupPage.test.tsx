import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LotView } from "@/generated/LotView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { FILE_INSTRUMENT, FILE_SOURCE } from "../fileTemplates/testTemplates";
import { closePanelWithEscape, panel, rowActionLabels } from "../panelTest";
import {
  ANALYTE,
  CONTROL_LOT,
  DRAFT_SOURCE,
  IDLE_INSTRUMENT,
  INSTRUMENT,
  REAGENT_LOT,
  sessionWith,
} from "../testData";
import { BenchSetupPage } from "./BenchSetupPage";

const WITHDRAWN_LOT: LotView = {
  ...CONTROL_LOT,
  lotId: "00000000-0000-7000-8000-000000003003",
  lotNumber: "CTL-000",
  state: "withdrawn",
  revision: 2,
};

function renderPage(
  path: string,
  capabilities: string[],
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_instrument_readiness: () => [INSTRUMENT, IDLE_INSTRUMENT],
    list_source_registrations: () => [DRAFT_SOURCE],
    list_analytes: () => [ANALYTE],
    list_lots: () => [CONTROL_LOT, REAGENT_LOT, WITHDRAWN_LOT],
    ...commands,
  });
  return renderRoutes(
    [
      { path: "/bench/setup", element: <BenchSetupPage /> },
      { path: "/bench/setup/:tab", element: <BenchSetupPage /> },
    ],
    path,
  );
}

const tab = (name: string) => screen.getByRole("tab", { name });

describe("BenchSetupPage (REQ-ANA-003, REQ-ANA-007)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the instruments tab at its base path, with status and approved sources", async () => {
    const { container } = renderPage("/bench/setup", ["perform_analysis"]);
    expect(await screen.findByText("Status not recorded")).toBeTruthy();
    expect(tab("Instruments").getAttribute("aria-selected")).toBe("true");
    expect(screen.getByText("SYN-SENDER 1.0")).toBeTruthy();
    expect(screen.getByText("None approved")).toBeTruthy();
    expect(screen.queryByRole("grid", { name: "Lots" })).toBeNull();
    // An analyst sees readiness but cannot change it.
    expect(screen.queryByRole("button", { name: `Actions for ${INSTRUMENT.name}` })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("follows the address from tab to tab", async () => {
    const { user, router } = renderPage("/bench/setup/lots", ["perform_analysis"]);
    expect(await screen.findByRole("grid", { name: "Lots" })).toBeTruthy();
    expect(tab("Lots").getAttribute("aria-selected")).toBe("true");
    await user.click(tab("Instruments"));
    expect(router.state.location.pathname).toBe("/bench/setup/instruments");
    expect(await screen.findByRole("grid", { name: "Instruments" })).toBeTruthy();
  });

  it("opens an instrument with its sources, and an approver approves a draft", async () => {
    const approve = vi.fn(() => ({ ...DRAFT_SOURCE, state: "active", allowedActions: [] }));
    const { user, router } = renderPage("/bench/setup", ["approve_configuration"], {
      approve_source: approve,
    });
    await openRow(user, INSTRUMENT.name);
    expect(router.state.location.search).toBe(`?open=${INSTRUMENT.instrumentId}`);
    const instrument = panel(INSTRUMENT.name);
    expect(await within(instrument).findByText("Draft, awaiting approval")).toBeTruthy();
    const codes = within(instrument).getByRole("list", { name: "Test codes of SYN-SENDER 2.0" });
    expect(codes.textContent).toContain("AMP → Synthetic amphetamine (SYN-AMP)");
    await user.click(within(instrument).getByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Approve SYN-SENDER 2.0" });
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    expect(await screen.findByText("SYN-SENDER 2.0 approved")).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: {
        registrationId: DRAFT_SOURCE.registrationId,
        expectedRevision: 1,
        selfApproval: false,
      },
    });
  });

  it("opens an instrument with Enter, and Escape closes it with focus back on the row", async () => {
    const { user } = renderPage("/bench/setup", ["perform_analysis"]);
    await openRowWithEnter(user, IDLE_INSTRUMENT.name);
    expect(within(panel(IDLE_INSTRUMENT.name)).getByText("Not recorded")).toBeTruthy();
    await closePanelWithEscape(user, IDLE_INSTRUMENT.name, IDLE_INSTRUMENT.name);
  });

  it("sets an instrument's status from its panel by keyboard, as its row menu offers", async () => {
    const setStatus = vi.fn(() => ({ ...INSTRUMENT, status: "out_of_service" }));
    const { user } = renderPage("/bench/setup", ["manage_quality", "manage_configuration"], {
      set_instrument_status: setStatus,
    });
    await screen.findByRole("grid", { name: "Instruments" });
    expect(await rowActionLabels(user, INSTRUMENT.name)).toEqual(["Set status", "Register source"]);
    await openRow(user, INSTRUMENT.name);
    const actions = within(panel(INSTRUMENT.name));
    expect(actions.getByRole("button", { name: "Register source" })).toBeTruthy();
    await tabTo(user, actions.getByRole("button", { name: "Set status" }));
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog", { name: `Set status of ${INSTRUMENT.name}` });
    await chooseWithKeyboard(user, /Status$/, "Out of service");
    await typeInto(user, /^Reason/, "Failed daily maintenance");
    await tabTo(user, within(dialog).getByRole("button", { name: "Set status" }));
    await user.keyboard("{Enter}");
    expect(await screen.findByText(`Status of ${INSTRUMENT.name} recorded`)).toBeTruthy();
    expect(setStatus).toHaveBeenCalledWith({
      input: {
        instrumentId: INSTRUMENT.instrumentId,
        status: "out_of_service",
        reason: "Failed daily maintenance",
      },
    });
  });

  it("requires a status and a reason", async () => {
    const setStatus = vi.fn();
    const { user } = renderPage("/bench/setup", ["manage_quality"], {
      set_instrument_status: setStatus,
    });
    await screen.findByRole("grid", { name: "Instruments" });
    const menu = await openRowActions(user, INSTRUMENT.name);
    await user.click(within(menu).getByRole("menuitem", { name: "Set status" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Set status" }));
    expect(await within(dialog).findByText("Choose status.")).toBeTruthy();
    expect(within(dialog).getByText("Enter the reason for this change.")).toBeTruthy();
    expect(setStatus).not.toHaveBeenCalled();
  });

  it("shows the engine's refusal of an approval by the source's author", async () => {
    const { user } = renderPage("/bench/setup", ["approve_configuration"], {
      approve_source: () =>
        Promise.reject(engineError("gate_blocked", "The author of a source cannot approve it.")),
    });
    await openRow(user, INSTRUMENT.name);
    await user.click(await screen.findByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    expect(
      await within(confirm).findByText("The author of a source cannot approve it."),
    ).toBeTruthy();
  });

  it("asks the source's author to confirm a self-approval the laboratory allows", async () => {
    const message =
      "You wrote this configuration. Your laboratory allows you to approve it yourself: confirm that no second person reviewed it.";
    const approve = vi
      .fn()
      .mockRejectedValueOnce(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
        }),
      )
      .mockReturnValue({ ...DRAFT_SOURCE, state: "active", allowedActions: [] });
    const { user } = renderPage("/bench/setup", ["approve_configuration"], {
      approve_source: approve,
    });
    await openRow(user, INSTRUMENT.name);
    await user.click(await screen.findByRole("button", { name: "Approve source" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    const checkbox = await within(confirm).findByRole("checkbox", { name: /no second person/ });
    expect(within(confirm).getByText(message)).toBeTruthy();
    expect(within(confirm).queryByText("Not recorded")).toBeNull();
    await user.click(checkbox);
    await user.click(within(confirm).getByRole("button", { name: "Approve source" }));
    expect(await screen.findByText("SYN-SENDER 2.0 approved")).toBeTruthy();
    expect(approve).toHaveBeenLastCalledWith({
      input: {
        registrationId: DRAFT_SOURCE.registrationId,
        expectedRevision: 1,
        selfApproval: true,
      },
    });
  });

  it("drafts a source with its test-code map from the instrument's panel", async () => {
    const register = vi.fn(() => DRAFT_SOURCE);
    const { user } = renderPage("/bench/setup", ["manage_configuration"], {
      register_source: register,
    });
    await openRow(user, INSTRUMENT.name);
    await user.click(
      within(panel(INSTRUMENT.name)).getByRole("button", { name: "Register source" }),
    );
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: /^Sender name/ }), "SYN-SENDER");
    await user.type(within(dialog).getByRole("textbox", { name: /^Sender version/ }), "2.0");
    await chooseWithKeyboard(user, /Sample ID is read from$/, "Specimen ID field");
    await chooseWithKeyboard(
      user,
      /Corrected results$/,
      "Corrections carry a corrected result status",
    );
    await chooseWithKeyboard(user, /Reports final concentration after dilution$/, "No");
    await user.click(within(dialog).getByRole("button", { name: "Add test code" }));
    await user.type(within(dialog).getByRole("textbox", { name: /^Source test code/ }), "AMP");
    await chooseWithKeyboard(user, /Analyte$/, "Synthetic amphetamine (SYN-AMP)");
    await user.click(within(dialog).getByRole("button", { name: "Register source" }));
    expect(await screen.findByText(/Source registered/)).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: {
        instrumentId: INSTRUMENT.instrumentId,
        senderName: "SYN-SENDER",
        senderVersion: "2.0",
        sampleIdField: "specimen_id",
        revisionIndicator: "result_status_correction",
        reportsFinalConcentration: false,
        testCodes: [{ sourceTestCode: "AMP", analyteId: ANALYTE.id }],
      },
    });
  });

  it("offers a result-file instrument a new file template instead of a source", async () => {
    const { user, router } = renderPage("/bench/setup", ["manage_configuration"], {
      list_instrument_readiness: () => [INSTRUMENT, FILE_INSTRUMENT],
      list_source_registrations: () => [],
    });
    await screen.findByRole("grid", { name: "Instruments" });
    expect(await rowActionLabels(user, INSTRUMENT.name)).toEqual(["Register source"]);
    expect(await rowActionLabels(user, FILE_INSTRUMENT.name)).toEqual(["New file template"]);
    await openRow(user, FILE_INSTRUMENT.name);
    await user.click(
      within(panel(FILE_INSTRUMENT.name)).getByRole("button", { name: "New file template" }),
    );
    expect(router.state.location.pathname).toBe(
      `/bench/setup/instruments/${FILE_INSTRUMENT.instrumentId}/file-template`,
    );
  });

  it("lists a file source with its software and shape; an approver checks it on its page", async () => {
    const { user, router } = renderPage("/bench/setup", ["approve_configuration"], {
      list_instrument_readiness: () => [FILE_INSTRUMENT],
      list_source_registrations: () => [FILE_SOURCE],
    });
    await openRow(user, FILE_INSTRUMENT.name);
    const instrument = panel(FILE_INSTRUMENT.name);
    expect(await within(instrument).findByText("Synthetic Quant 12.1")).toBeTruthy();
    expect(within(instrument).getByText("Long, one row per sample and analyte")).toBeTruthy();
    expect(within(instrument).queryByRole("button", { name: "Approve source" })).toBeNull();
    await user.click(within(instrument).getByRole("button", { name: "Check and approve" }));
    expect(router.state.location.pathname).toBe(
      `/bench/setup/instruments/${FILE_INSTRUMENT.instrumentId}/sources/${FILE_SOURCE.registrationId}`,
    );
  });

  it("opens an approved file source's template for anyone", async () => {
    const { user } = renderPage("/bench/setup", ["perform_analysis"], {
      list_instrument_readiness: () => [FILE_INSTRUMENT],
      list_source_registrations: () => [{ ...FILE_SOURCE, state: "active", allowedActions: [] }],
    });
    await openRow(user, FILE_INSTRUMENT.name);
    expect(
      await within(panel(FILE_INSTRUMENT.name)).findByRole("button", { name: "Open template" }),
    ).toBeTruthy();
  });

  it("hides withdrawn and expired lots until asked, keeping the choice in the address", async () => {
    const { user, router, container } = renderPage("/bench/setup/lots", ["perform_analysis"]);
    const grid = await screen.findByRole("grid", { name: "Lots" });
    expect(within(grid).getByRole("gridcell", { name: "CTL-001" })).toBeTruthy();
    expect(within(grid).queryByRole("gridcell", { name: "RGT-009" })).toBeNull();
    expect(within(grid).queryByRole("gridcell", { name: "CTL-000" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await chooseWithKeyboard(user, /Show$/, "All lots, with withdrawn and expired");
    expect(router.state.location.search).toBe("?show=all");
    expect(await screen.findByText("Expired")).toBeTruthy();
    expect(screen.getByText("Withdrawn")).toBeTruthy();
  });

  it("says when the filter hides every lot, and clears it", async () => {
    const { user } = renderPage("/bench/setup/lots", ["perform_analysis"], {
      list_lots: () => [REAGENT_LOT, WITHDRAWN_LOT],
    });
    expect(await screen.findByText("No lots match the filters")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(await screen.findByRole("gridcell", { name: "RGT-009" })).toBeTruthy();
  });

  it("registers a lot and opens it, then withdraws an active one with a reason", async () => {
    const register = vi.fn(() => CONTROL_LOT);
    const withdraw = vi.fn(() => ({ ...CONTROL_LOT, state: "withdrawn" }));
    const { user, router } = renderPage("/bench/setup/lots", ["manage_quality"], {
      register_lot: register,
      withdraw_lot: withdraw,
    });
    await user.click(await screen.findByRole("button", { name: "Register lot" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Material$/, "Control");
    await user.type(within(dialog).getByRole("textbox", { name: /^Product/ }), "Synthetic control");
    await user.type(within(dialog).getByRole("textbox", { name: /^Lot number/ }), "CTL-002");
    await user.type(within(dialog).getByRole("textbox", { name: /^Sample label/ }), "QC-L2");
    await user.type(within(dialog).getByRole("textbox", { name: /^Expires on/ }), "2027-02-28");
    await user.click(within(dialog).getByRole("button", { name: "Register lot" }));
    expect(await screen.findByText("Lot registered")).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: {
        materialKind: "control",
        productName: "Synthetic control",
        lotNumber: "CTL-002",
        sampleLabel: "QC-L2",
        expiresOn: "2027-02-28",
      },
    });
    expect(router.state.location.search).toBe(`?open=${CONTROL_LOT.lotId}`);

    expect(await rowActionLabels(user, "CTL-001")).toEqual(["Withdraw lot"]);
    await user.click(within(panel("Lot CTL-001")).getByRole("button", { name: "Withdraw lot" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Withdraw lot CTL-001" });
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Contaminated");
    await user.click(within(confirm).getByRole("button", { name: "Withdraw lot" }));
    expect(await screen.findByText("Lot CTL-001 withdrawn")).toBeTruthy();
    expect(withdraw).toHaveBeenCalledWith({
      input: { lotId: CONTROL_LOT.lotId, expectedRevision: 1, reason: "Contaminated" },
    });
  });

  it("offers no withdrawal of a withdrawn lot, and none to analysts", async () => {
    const { user } = renderPage(`/bench/setup/lots?show=all&open=${WITHDRAWN_LOT.lotId}`, [
      "manage_quality",
    ]);
    const lot = await screen.findByRole("region", { name: "Lot CTL-000" });
    expect(within(lot).queryByRole("button", { name: "Withdraw lot" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for CTL-000" })).toBeNull();
    await closePanelWithEscape(user, "Lot CTL-000", "CTL-000");
  });

  it("shows an analyst the lots without any change", async () => {
    const { user } = renderPage("/bench/setup/lots", ["perform_analysis"]);
    await openRow(user, "CTL-001");
    expect(within(panel("Lot CTL-001")).getByText("QC-L1")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Withdraw lot" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Register lot" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for CTL-001" })).toBeNull();
  });

  it("shows loading while the engine answers", async () => {
    renderPage("/bench/setup", ["perform_analysis"], {
      list_instrument_readiness: () => new Promise(() => undefined),
    });
    expect(await screen.findByText("Loading instruments…")).toBeTruthy();
  });

  it("says when nothing is registered, and shows the engine's error", async () => {
    const { user } = renderPage("/bench/setup", ["perform_analysis"], {
      list_instrument_readiness: () => [],
      list_lots: () => Promise.reject(engineError("internal", "The lots could not be read.")),
    });
    expect(await screen.findByText("No instruments yet")).toBeTruthy();
    await user.click(tab("Lots"));
    expect(await screen.findByText("The lots could not be read.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("says when no lot is registered yet", async () => {
    renderPage("/bench/setup/lots", ["manage_quality"], { list_lots: () => [] });
    expect(await screen.findByText("No lots yet")).toBeTruthy();
  });
});
