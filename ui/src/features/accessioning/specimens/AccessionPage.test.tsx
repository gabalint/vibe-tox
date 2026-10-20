import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AccessionDetail } from "@/generated/AccessionDetail";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRowActions, selectRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import {
  ACCESSION,
  ACCESSION_ID,
  container,
  ORDER,
  ORDER_ID,
  SUBJECT_VIEW,
  sessionWith,
} from "../testData";
import { AccessionPage } from "./AccessionPage";

const PRINTERS = [
  {
    printerId: "printer-1",
    code: "bench-1",
    name: "Bench printer",
    transport: "tcp",
    ipAddress: "192.0.2.10",
    port: 9100,
    queue: null,
    active: true,
    revision: 1,
  },
  {
    printerId: "printer-2",
    code: "old-1",
    name: "Retired printer",
    transport: "cups",
    ipAddress: null,
    port: null,
    queue: "retired_labels",
    active: false,
    revision: 1,
  },
  {
    printerId: "printer-3",
    code: "cups-1",
    name: "Queue printer",
    transport: "cups",
    ipAddress: null,
    port: null,
    queue: "toxicology_labels",
    active: true,
    revision: 1,
  },
];

function withContainers(containers: AccessionDetail["specimens"][number]["containers"]) {
  return {
    ...ACCESSION,
    specimens: ACCESSION.specimens.map((specimen) => ({ ...specimen, containers })),
  };
}

function renderPage(
  accession: AccessionDetail | (() => unknown),
  commands: Record<string, (args: unknown) => unknown> = {},
) {
  const getAccession = vi.fn(typeof accession === "function" ? accession : () => accession);
  mockCommands({
    current_session: () => sessionWith(["accession", "enter_orders"]),
    get_accession: getAccession,
    get_order: () => ORDER,
    get_subject: () => SUBJECT_VIEW,
    list_printers: () => PRINTERS,
    ...commands,
  });
  const result = renderRoutes(
    [{ path: "/accessions/:accessionId", element: <AccessionPage /> }],
    `/accessions/${ACCESSION_ID}`,
  );
  return { ...result, getAccession };
}

type User = ReturnType<typeof renderPage>["user"];

/** Opens the receipt decision and scans the container's label, as a scanner would. */
async function startReceiptDecision(user: User) {
  await user.click(await screen.findByRole("button", { name: "Record receipt decision" }));
  await screen.findByRole("dialog", { name: "Receipt decision for Specimen 1 (urine)" });
  await user.keyboard("C00000001{Enter}");
  return screen.findByRole("dialog", { name: "Receipt decision for Specimen 1 (urine)" });
}

describe("AccessionPage (REQ-SPC-001 to REQ-SPC-003)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the collection as recorded, the specimen and its containers", async () => {
    const { container: page, getAccession } = renderPage(ACCESSION);
    expect(screen.getByRole("heading", { level: 1, name: "Accession" })).toBeTruthy();
    expect(screen.getByText("Loading accession…")).toBeTruthy();
    expect(await screen.findByRole("heading", { name: "Accession A26000001" })).toBeTruthy();
    expect(getAccession).toHaveBeenCalledWith({ input: { accessionId: ACCESSION_ID } });
    expect(screen.getByText("2026-10-12 13:30 UTC (site clock UTC−05:00)")).toBeTruthy();
    expect(screen.getByText("Received, not yet decided")).toBeTruthy();
    expect(screen.getByText("30.0 mL, estimated")).toBeTruthy();
    expect(screen.getByText("No label requested")).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Order PO-0001" })).toHaveProperty(
      "href",
      expect.stringContaining(`/orders/${ORDER_ID}`),
    );
    expect(screen.getByRole("link", { name: "Order" })).toHaveProperty(
      "href",
      expect.stringContaining(`/orders/${ORDER_ID}`),
    );
    expect(screen.getByRole("list", { name: "Tests in this accession" }).textContent).toContain(
      "SYN-SCR Synthetic drug screen",
    );
    expect(screen.getByText("0 of 1 recorded")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Print labels" })).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient and specimen identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    expect(within(banner).getByText("A26000001")).toBeTruthy();
    // Two identifiers on every container row: the label and the accession.
    const grid = screen.getByRole("grid", { name: "Containers of Specimen 1 (urine)" });
    const row = within(grid).getByRole("gridcell", { name: "C00000001" }).closest("[role=row]");
    if (!(row instanceof HTMLElement)) throw new Error("no row");
    expect(within(row).getByRole("gridcell", { name: "A26000001" })).toBeTruthy();
    expect(screen.queryByText(/Select containers/)).toBeNull();
    await expectNoA11yViolations(page, { wholePage: true });
  });

  it("opens label printing directly without selecting a container row", async () => {
    const { user } = renderPage(ACCESSION);
    await user.click(await screen.findByRole("button", { name: "Print labels" }));
    const dialog = await screen.findByRole("dialog", { name: "Print labels" });
    expect(within(dialog).getByText("Containers: C00000001")).toBeTruthy();
  });

  it("explains missing printer setup without sending an accessioner to configuration", async () => {
    const { user } = renderPage(ACCESSION, { list_printers: () => [] });
    await user.click(await screen.findByRole("button", { name: "Print labels" }));
    expect(await screen.findByText("No active label printer is set up")).toBeTruthy();
    expect(
      screen.getByText("An administrator adds label printers before labels can be sent."),
    ).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Configure label printers" })).toBeNull();
  });

  it("does not read or link the order without order-entry capability", async () => {
    const getOrder = vi.fn(() => ORDER);
    renderPage(ACCESSION, {
      current_session: () => sessionWith(["accession"]),
      get_order: getOrder,
    });
    expect(await screen.findByRole("heading", { name: "Finish receiving" })).toBeTruthy();
    expect(screen.getByText("0 of 1 recorded")).toBeTruthy();
    expect(getOrder).not.toHaveBeenCalled();
    expect(screen.queryByRole("link", { name: /^Order/ })).toBeNull();
  });

  it("lets quality read an accession without offering accession or billing mutations", async () => {
    renderPage(ACCESSION, { current_session: () => sessionWith(["manage_quality"]) });
    expect(await screen.findByRole("heading", { name: "Finish receiving" })).toBeTruthy();
    expect(
      screen.queryByRole("button", {
        name: /Print labels|Record receipt decision|Open order billing/,
      }),
    ).toBeNull();
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(
      screen.getByRole("link", { name: "Material history and actions: C00000001" }),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "Collection timing review" })).toBeTruthy();
  });

  it("shows held receipt progress without claiming the material is ready for testing", async () => {
    renderPage({
      ...ACCESSION,
      specimens: ACCESSION.specimens.map((specimen) => ({ ...specimen, acceptance: "held" })),
    });
    expect(await screen.findByRole("heading", { name: "Receipt decisions recorded" })).toBeTruthy();
    expect(screen.getByText("1 of 1 recorded")).toBeTruthy();
    expect(screen.getByText("Held specimens need review under laboratory policy.")).toBeTruthy();
    expect(screen.queryByText(/ready for testing/i)).toBeNull();
  });

  it("shows a date-only or unknown collection time as such", async () => {
    renderPage({
      ...ACCESSION,
      collection: { ...ACCESSION.collection, precision: "unknown", collectedAt: null },
    });
    expect(await screen.findByText("Unknown")).toBeTruthy();
  });

  it("names label outcomes by evidence level and never as printed", async () => {
    const jobs = (outcome: "handed_off" | "failed" | null, kind: "print" | "reprint") => ({
      jobId: `job-${outcome}`,
      containerId: "c",
      printerId: "printer-1",
      kind,
      requestedAt: "2026-10-12T15:01:00Z",
      outcome,
      errorCode: outcome === "failed" ? "printer_unreachable" : null,
    });
    renderPage(
      withContainers([
        container({ labelCode: "C00000001", labelJobs: [jobs(null, "print")] }),
        container({
          containerId: "c2",
          labelCode: "C00000002",
          labelJobs: [jobs("handed_off", "print")],
        }),
        container({
          containerId: "c3",
          labelCode: "C00000003",
          labelJobs: [jobs("failed", "reprint")],
        }),
      ]),
    );
    expect(await screen.findByText("Print: Queued")).toBeTruthy();
    expect(screen.getByText("Print: Handed off to printer")).toBeTruthy();
    expect(screen.getByText("Reprint: Failed (printer_unreachable)")).toBeTruthy();
    expect(screen.queryByText(/printed/i)).toBeNull();
  });

  it("says why a label failed in laboratory words, and an unknown code as sent", async () => {
    const failed = (containerId: string, errorCode: string) =>
      container({
        containerId,
        labelCode: `C-${containerId}`,
        labelJobs: [
          {
            jobId: `job-${containerId}`,
            containerId,
            printerId: "printer-1",
            kind: "print",
            requestedAt: "2026-10-12T15:01:00Z",
            outcome: "failed",
            errorCode,
          },
        ],
      });
    renderPage(
      withContainers([
        failed("c1", "printer_inactive"),
        failed("c2", "spooler_timeout"),
        failed("c3", "synthetic_new_code"),
      ]),
    );
    expect(await screen.findByText("Print: Failed (Printer not in service)")).toBeTruthy();
    expect(screen.getByText("Print: Failed (Print service did not answer)")).toBeTruthy();
    expect(screen.getByText("Print: Failed (synthetic_new_code)")).toBeTruthy();
    expect(screen.queryByText(/printer_inactive/)).toBeNull();
  });

  it("records a receipt decision with discrepancies and a hold reason", async () => {
    const record = vi.fn(() => ACCESSION);
    const { user } = renderPage(ACCESSION, { record_receipt_decision: record });
    const dialog = await startReceiptDecision(user);
    expect(within(dialog).getByText("Container C00000001 checked.")).toBeTruthy();
    await chooseWithKeyboard(user, /Decision$/, "Hold");
    await user.click(within(dialog).getByRole("checkbox", { name: "Leaking" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Note on leaking" }), "Cap loose");
    await user.click(within(dialog).getByRole("button", { name: "Record decision" }));
    expect(await within(dialog).findByText("Enter reason.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Ask the clinic");
    await user.click(within(dialog).getByRole("button", { name: "Record decision" }));
    expect(await screen.findByText(/Receipt decision recorded/)).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        specimenId: ACCESSION.specimens[0]?.specimenId,
        expectedRevision: 1,
        decision: "held",
        discrepancies: [{ code: "leaking", note: "Cap loose" }],
        reason: "Ask the clinic",
      },
    });
  });

  it("keeps the decision closed until a scanned or typed label matches one of the specimen's containers", async () => {
    const record = vi.fn(() => ACCESSION);
    const { user } = renderPage(ACCESSION, { record_receipt_decision: record });
    await user.click(await screen.findByRole("button", { name: "Record receipt decision" }));
    const check = await screen.findByRole("dialog", {
      name: "Receipt decision for Specimen 1 (urine)",
    });
    const field = within(check).getByRole("textbox", { name: /^Container label/ });
    expect(document.activeElement).toBe(field);
    expect(screen.queryByRole("button", { name: "Record decision" })).toBeNull();

    // A label of another container, scanned with Enter, is refused and cleared for a rescan.
    await user.keyboard("C00000099{Enter}");
    expect(
      await within(check).findByText(
        "C00000099 is not a container of Specimen 1 (urine). Check the container in your hand.",
      ),
    ).toBeTruthy();
    expect(field).toHaveProperty("value", "");
    expect(screen.queryByRole("button", { name: "Record decision" })).toBeNull();
    // The match is exact: a different case is a different label.
    await user.keyboard("c00000001{Enter}");
    expect(await within(check).findByText(/^c00000001 is not a container/)).toBeTruthy();

    await user.keyboard("C00000001{Enter}");
    expect(await screen.findByRole("button", { name: "Record decision" })).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
  });

  it("prints labels for several containers from the selection", async () => {
    const print = vi.fn(() => []);
    const { user } = renderPage(
      withContainers([
        container(),
        container({ containerId: "c2", labelCode: "C00000002" }),
        container({ containerId: "c3", labelCode: "C00000003", allowedActions: [] }),
      ]),
      { print_labels: print },
    );
    await selectRow(user, "C00000001");
    await selectRow(user, "C00000002");
    expect(screen.getByText(/^2 selected/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Print labels" }));
    const dialog = await screen.findByRole("dialog", { name: "Print labels" });
    expect(within(dialog).getByText("Containers: C00000001, C00000002")).toBeTruthy();
    await user.click(await within(dialog).findByRole("button", { name: /Printer$/ }));
    await user.click(
      screen.getByRole("option", { name: "Bench printer (bench-1 — 192.0.2.10:9100)" }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Print labels" }));
    await vi.waitFor(() =>
      expect(print).toHaveBeenCalledWith({
        input: {
          containerIds: ["00000000-0000-7000-8000-000000001401", "c2"],
          printerId: "printer-1",
          kind: "print",
          reason: undefined,
        },
      }),
    );

    // A container the engine does not let print takes printing out of the selection's actions.
    await selectRow(user, "C00000003");
    expect(screen.getByText(/^3 selected/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Print labels" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Actions for C00000003" })).toBeNull();
  });

  it("requires the policy and scope for a conditional acceptance", async () => {
    const record = vi.fn(() => ACCESSION);
    const { user } = renderPage(ACCESSION, { record_receipt_decision: record });
    const dialog = await startReceiptDecision(user);
    await chooseWithKeyboard(user, /Decision$/, "Accept conditionally");
    await user.click(within(dialog).getByRole("button", { name: "Record decision" }));
    expect(await within(dialog).findByText("Enter laboratory policy reference.")).toBeTruthy();
    expect(within(dialog).getByText("Enter scope of the condition.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
  });

  it("confirms a rejection separately, and can go back", async () => {
    const record = vi.fn(() => ACCESSION);
    const { user } = renderPage(ACCESSION, { record_receipt_decision: record });
    await startReceiptDecision(user);
    await chooseWithKeyboard(user, /Decision$/, "Reject");
    await user.type(screen.getByRole("textbox", { name: /^Reason/ }), "Unlabeled cup");
    await user.click(screen.getByRole("button", { name: "Record decision" }));
    const confirm = await screen.findByRole("alertdialog", { name: /^Reject Specimen 1/ });
    expect(within(confirm).getByText(/cannot be changed afterwards/)).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Reject specimen" }));
    expect(await screen.findByText(/Receipt decision recorded/)).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: expect.objectContaining({ decision: "rejected", reason: "Unlabeled cup" }),
    });
  });

  it("creates an aliquot from a container's actions, once its label is checked", async () => {
    const aliquot = vi.fn(() => ACCESSION);
    const { user } = renderPage(ACCESSION, { create_aliquot: aliquot });
    const menu = await openRowActions(user, "C00000001");
    await user.click(within(menu).getByRole("menuitem", { name: "Create aliquot" }));
    const check = await screen.findByRole("dialog", { name: "Create aliquot from C00000001" });
    expect(within(check).queryByRole("button", { name: "Create aliquot" })).toBeNull();
    await user.type(within(check).getByRole("textbox", { name: /^Container label/ }), "C00000001");
    await user.click(within(check).getByRole("button", { name: "Check label" }));
    const dialog = await screen.findByRole("dialog", { name: "Create aliquot from C00000001" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Volume taken/ }), "1.50");
    await user.type(within(dialog).getByRole("textbox", { name: /^Volume unit/ }), "mL");
    await chooseWithKeyboard(user, /Volume taken is$/, "Measured");
    await user.click(within(dialog).getByRole("button", { name: "Create aliquot" }));
    expect(await screen.findByText("Aliquot created from C00000001")).toBeTruthy();
    expect(aliquot).toHaveBeenCalledWith({
      input: {
        parentContainerId: "00000000-0000-7000-8000-000000001401",
        expectedRevision: 1,
        volumeTaken: "1.50",
        volumeUnit: "mL",
        volumeKind: "measured",
      },
    });
  });

  it("does not offer an aliquot the engine does not allow", async () => {
    const { user } = renderPage(
      withContainers([
        container({
          allowedActions: ["print_labels"],
          openHolds: [
            {
              holdId: "h",
              containerId: "00000000-0000-7000-8000-000000001401",
              kind: "quality",
              state: "open",
              excursionId: null,
              reason: "Synthetic hold",
              placedAt: "2026-10-12T15:02:00Z",
              revision: 1,
            },
          ],
        }),
      ]),
    );
    expect(await screen.findByText("1 open")).toBeTruthy();
    const menu = await openRowActions(user, "C00000001");
    expect(within(menu).getByRole("menuitem", { name: "Print label" })).toBeTruthy();
    expect(within(menu).queryByRole("menuitem", { name: "Create aliquot" })).toBeNull();
  });

  it("prints to an active printer, and requires a reason for a reprint", async () => {
    const print = vi.fn(() => [
      {
        jobId: "job-1",
        containerId: "c",
        kind: "reprint",
        requestedAt: "2026-10-12T15:01:00Z",
        outcome: null,
        errorCode: null,
      },
    ]);
    const { user, getAccession } = renderPage(ACCESSION, { print_labels: print });
    await selectRow(user, "C00000001");
    expect(screen.getByText(/^1 selected/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Reprint labels" }));
    const dialog = await screen.findByRole("dialog", { name: "Reprint labels" });
    await user.click(await within(dialog).findByRole("button", { name: /Printer$/ }));
    expect(screen.queryByRole("option", { name: /Retired printer/ })).toBeNull();
    expect(
      screen.getByRole("option", { name: "Queue printer (cups-1 — queue toxicology_labels)" }),
    ).toBeTruthy();
    await user.click(
      screen.getByRole("option", { name: "Bench printer (bench-1 — 192.0.2.10:9100)" }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Reprint labels" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Label smudged");
    await user.click(within(dialog).getByRole("button", { name: "Reprint labels" }));
    expect(await screen.findByText("1 label job queued")).toBeTruthy();
    expect(print).toHaveBeenCalledWith({
      input: {
        containerIds: ["00000000-0000-7000-8000-000000001401"],
        printerId: "printer-1",
        kind: "reprint",
        reason: "Label smudged",
      },
    });

    // Outcomes come from the printer worker; the screen asks for them on request.
    await user.click(screen.getByRole("button", { name: "Check label status" }));
    expect(getAccession.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it("keeps focus on Next while the next accession of the list loads", async () => {
    const ids = [ACCESSION_ID, "accession-2", "accession-3"];
    mockCommands({
      get_accession: (args) => {
        const index = ids.findIndex((id) => JSON.stringify(args).includes(id));
        return { ...ACCESSION, accessionId: ids[index], accessionNumber: `A2600000${index + 1}` };
      },
      get_subject: () => SUBJECT_VIEW,
      get_order: () => ORDER,
    });
    const { user } = await renderFromList({
      route: { path: "/accessions/:accessionId", element: <AccessionPage /> },
      recordPath: (id) => `/accessions/${id}`,
      ids,
    });
    expect(
      await screen.findByRole("heading", { level: 1, name: "Accession A26000001" }),
    ).toBeTruthy();
    await expectNextKeepsFocus(user, "Accession A26000002");
  });

  it("shows the engine error with a retry", async () => {
    renderPage(() => Promise.reject(engineError("not_found", "The accession was not found.")));
    expect(await screen.findByText("The accession was not found.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Accession" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
});
