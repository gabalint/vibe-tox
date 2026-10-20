import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DerivationView } from "@/generated/DerivationView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import { derivationPath } from "../paths";
import { DERIVATION, DERIVATION_ID, SUBJECT, sessionWith } from "../testData";
import { DerivationPage } from "./DerivationPage";

const CANDIDATE: DerivationView = {
  ...DERIVATION,
  state: "candidate",
  flags: [{ code: "ordered_not_performed", refId: "", blocking: false }],
  adjustmentOf: null,
  allowedActions: ["approve_service_derivation", "derive_services"],
};

const ROUTE = { path: "/billing/derivations/:derivationId", element: <DerivationPage /> };

function mockPage(
  derivation: DerivationView,
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockCommands({
    current_session: () => sessionWith(["billing"]),
    get_service_derivation: () => derivation,
    ...commands,
  });
}

function renderPage(
  derivation: DerivationView,
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  mockPage(derivation, commands);
  return renderRoutes([ROUTE], `/billing/derivations/${DERIVATION_ID}`);
}

describe("DerivationPage (REQ-BIL-005)", () => {
  beforeEach(mockGridLayout);

  it("records explicit same-day context reconciliation with the reviewed derivation revision", async () => {
    const resolve = vi.fn(() => ({}));
    const { user } = renderPage(
      { ...DERIVATION, allowedActions: ["resolve_billing_context_conflict"] },
      { resolve_billing_context_conflict: resolve },
    );
    await user.click(
      await screen.findByRole("button", { name: "Resolve billing context conflict" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Resolve billing context conflict" });
    expect(within(dialog).getByText(/does not create additional services or claims/)).toBeTruthy();
    await user.type(
      within(dialog).getByRole("textbox", { name: /Allocation decision, evidence and reason/ }),
      "Synthetic billing owner confirmed allocation for both orders",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record reconciliation" }));
    expect(await screen.findByText("Billing context reconciliation recorded")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        derivationId: DERIVATION.derivationId,
        expectedRevision: DERIVATION.revision,
        reason: "Synthetic billing owner confirmed allocation for both orders",
      },
    });
  });

  it("keeps focus on Next while the next derivation loads", async () => {
    const second = "00000000-0000-7000-8000-0000000000d2";
    mockPage(DERIVATION, {
      get_service_derivation: (args) =>
        JSON.stringify(args).includes(second)
          ? { ...DERIVATION, derivationId: second, derivationNo: 3 }
          : DERIVATION,
    });
    const { user } = await renderFromList({
      route: ROUTE,
      recordPath: derivationPath,
      ids: [DERIVATION_ID, second, "00000000-0000-7000-8000-0000000000d3"],
    });
    await screen.findByRole("heading", { level: 1, name: "Service derivation 2" });
    await expectNextKeepsFocus(user, "Service derivation 3");
  });

  it("shows a held derivation's blocking flags, classes and dispositions", async () => {
    const { container } = renderPage(DERIVATION);
    expect(
      await screen.findByRole("heading", { level: 1, name: "Service derivation 2" }),
    ).toBeTruthy();
    expect(
      screen.getByText("1 blocking flag holds this derivation.", { exact: false }),
    ).toBeTruthy();
    const flags = screen.getByRole("grid", { name: "Flags" });
    expect(
      within(flags).getByRole("gridcell", { name: "No billing class for an analyte" }),
    ).toBeTruthy();
    expect(within(flags).getByRole("gridcell", { name: "Synthetic opiate analyte" })).toBeTruthy();
    expect(
      within(flags).getByRole("gridcell", { name: "Blocking: holds the derivation" }),
    ).toBeTruthy();
    expect(within(flags).getByRole("gridcell", { name: "For information" })).toBeTruthy();
    const classes = screen.getByRole("grid", { name: "Billing classes for accession A26000001" });
    const row = within(classes).getByRole("gridcell", { name: "SYN-C1" }).closest("[role=row]");
    expect(row?.textContent).toContain("Presumptive (screening)YesYesNo");
    expect(screen.getByRole("gridcell", { name: "Failed attempts only" })).toBeTruthy();
    // The identity banner under the title; its medical record number is the bold identifier.
    const identity = screen.getByRole("region", { name: "Patient identity" });
    expect(identity.textContent).toContain("Patient, Synthetic");
    expect(within(identity).getByText("MRN-00000").tagName).toBe("STRONG");
    // The held alert has the hold tone, as the held state's badge does.
    expect(screen.getByRole("alert").getAttribute("data-tone")).toBe("hold");
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByRole("link", { name: "Billing work" }).getAttribute("href")).toBe(
      "/billing/work",
    );
    expect(screen.getByRole("link", { name: "Derivation 1" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Version 1" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve derivation" })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("approves a candidate with its revision", async () => {
    const approve = vi.fn(() => ({}));
    const { user } = renderPage(CANDIDATE, { approve_service_derivation: approve });
    await user.click(await screen.findByRole("button", { name: "Approve derivation" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Approve derivation" }));
    expect(await screen.findByText("Derivation 2 approved")).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: { derivationId: DERIVATION_ID, expectedRevision: 4 },
    });
  });

  it("shows the engine's refusal to approve a stale derivation", async () => {
    const { user } = renderPage(
      { ...CANDIDATE, stale: true },
      {
        approve_service_derivation: () =>
          Promise.reject(engineError("conflict", "The derivation's inputs have changed.")),
      },
    );
    expect(await screen.findByText("Out of date")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Approve derivation" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Approve derivation" }));
    expect(await within(dialog).findByText("The derivation's inputs have changed.")).toBeTruthy();
  });

  it("derives again and opens the new derivation", async () => {
    const derive = vi.fn(() => ({
      ...CANDIDATE,
      derivationId: "00000000-0000-7000-8000-0000000000d5",
    }));
    const { user, router } = renderPage(CANDIDATE, { derive_services: derive });
    await user.click(await screen.findByRole("button", { name: "Derive again" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Derive services" }));
    await screen.findByText("Services derived");
    expect(router.state.location.pathname).toBe(
      "/billing/derivations/00000000-0000-7000-8000-0000000000d5",
    );
    expect(derive).toHaveBeenCalledWith({
      input: { subjectId: SUBJECT.subjectId, dateOfService: "2026-10-10" },
    });
  });

  it("shows the engine's error", async () => {
    renderPage(DERIVATION, {
      get_service_derivation: () => Promise.reject(engineError("not_found", "No such derivation.")),
    });
    expect(await screen.findByText("No such derivation.")).toBeTruthy();
  });
});
