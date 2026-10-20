import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ClientView } from "@/generated/ClientView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { pickRecord } from "@/lib/test/picker";
import { renderRoutes } from "@/lib/test/router";
import {
  CLIENTS,
  LARGE_CATALOG,
  LONG_CATALOG,
  ORDER,
  ORDER_ID,
  ORDERABLES,
  PRACTITIONERS,
  SCREEN,
  SUBJECT_ID,
  SUBJECT_VIEW,
  SUMMARY,
  sessionWith,
} from "../testData";
import { NewOrderPage } from "./NewOrderPage";

const TWO_CLIENTS: ClientView[] = [
  ...CLIENTS,
  {
    id: "00000000-0000-7000-8000-000000000611",
    active: true,
    revision: 1,
    code: "syn-south",
    name: "Synthetic Clinic South",
    locations: [
      {
        id: "00000000-0000-7000-8000-000000000612",
        code: "south",
        name: "South Office",
        active: true,
        revision: 1,
      },
    ],
  },
];

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}, path?: string) {
  mockCommands({
    current_session: () => sessionWith(["enter_orders", "accession"]),
    get_subject: () => SUBJECT_VIEW,
    search_subjects: () => [SUMMARY],
    list_clients: () => CLIENTS,
    list_practitioners: () => PRACTITIONERS,
    list_active_orderables: () => ORDERABLES,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/orders/new", element: <NewOrderPage /> }],
    path ?? `/orders/new?subjectId=${SUBJECT_ID}`,
  );
}

describe("NewOrderPage (REQ-ORD-001, REQ-ORD-005, REQ-ORD-007)", {
  timeout: WALKTHROUGH_TIMEOUT_MS,
}, () => {
  beforeEach(mockGridLayout);

  it("records a requisition with the keyboard only, exactly as entered", async () => {
    const register = vi.fn(() => ORDER);
    const { user, container } = renderPage({ register_order: register });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await expectNoA11yViolations(container, { wholePage: true });

    await pickRecord(user, /^Client/, "syn-clinic");
    await pickRecord(user, /^Ordering practitioner/, "0000000000");
    await chooseWithKeyboard(user, /Received as/, "Paper requisition");
    await user.click(screen.getByText("Clinic order number and source references"));
    await typeInto(user, /^Clinic order number$/, "PO-0001");
    await chooseWithKeyboard(user, /Billing responsibility/, "Not supplied");
    await chooseWithKeyboard(user, /Template use/, "No template");
    await tabTo(user, screen.getByRole("checkbox", { name: /SYN-SCR/ }));
    await user.keyboard(" ");
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Not stated");
    await chooseWithKeyboard(user, /Authority to test/, "Handwritten signature");
    await tabTo(user, screen.getByRole("button", { name: "Add diagnosis" }));
    await user.keyboard("{Enter}");
    await typeInto(user, /^Code\b(?! system)/, "Z00.00");
    await typeInto(user, /^Code system\b(?! version)/, "ICD-10-CM");
    await typeInto(user, "Source document", "REQ-SCAN-0001");
    await chooseWithKeyboard(user, /Pre-printed on the laboratory's form/, "No");
    await tabTo(user, screen.getByRole("button", { name: "Save order" }));
    await user.keyboard("{Enter}");

    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}`)).toBeTruthy();
    expect(register).toHaveBeenCalledWith({
      input: {
        subjectId: SUBJECT_ID,
        clientId: CLIENTS[0]?.id,
        clientLocationId: null,
        practitionerId: PRACTITIONERS[0]?.id,
        placerOrderId: "PO-0001",
        placerAuthority: null,
        sourceKind: "paper_requisition",
        sourceDocumentRef: null,
        billingResponsibility: "not_supplied",
        facilitySetting: null,
        standingOrderId: null,
        templateUse: { kind: "none" },
        items: [
          {
            orderableVersionId: SCREEN.id,
            sequenceRole: "first_line",
            reflexAuthorization: "not_stated",
            reflexScopeOrderableIds: [],
          },
        ],
        evidence: {
          authority: { kind: "wet_signature", documentRef: null },
          diagnoses: [
            {
              code: "Z00.00",
              codeSystem: "ICD-10-CM",
              codeSystemVersion: null,
              documentRef: "REQ-SCAN-0001",
              preprintedOnLabForm: false,
            },
          ],
          medications: [],
          pointOfCareResults: [],
        },
      },
    });
  });

  it("prefills nothing: every required element must be entered", async () => {
    const register = vi.fn();
    const { user } = renderPage({ register_order: register });
    await user.click(await screen.findByRole("button", { name: "Save order" }));
    expect(await screen.findByText("Choose client.")).toBeTruthy();
    for (const message of [
      "Choose ordering practitioner.",
      "Choose received as.",
      "Choose billing responsibility.",
      "Choose template use.",
      "Choose at least one test.",
      "Choose authority to test.",
    ]) {
      expect(screen.getByText(message)).toBeTruthy();
    }
    expect(register).not.toHaveBeenCalled();
  });

  it("records a clinic template, reflex scope, medication and point-of-care result as supplied", async () => {
    const register = vi.fn((_args: unknown) => ORDER);
    const { user } = renderPage({ register_order: register });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await pickRecord(user, /^Client/, "Synthetic");
    await chooseWithKeyboard(user, /Client location/, "North Office");
    await pickRecord(user, /^Ordering practitioner/, "Doctor");
    await chooseWithKeyboard(user, /Received as/, "Fax");
    await chooseWithKeyboard(user, /Billing responsibility/, "Patient (self-pay)");
    await chooseWithKeyboard(user, /Template use/, "Clinic template");
    await typeInto(user, /^Template name/, "Synthetic panel form");
    await chooseWithKeyboard(user, /Confirmed for this patient/, "Yes");
    await user.click(screen.getByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Authorized");
    const scope = screen.getByRole("group", { name: /Reflex tests authorized/ });
    await user.click(within(scope).getByRole("checkbox", { name: /SYN-CNF/ }));
    await chooseWithKeyboard(user, /Authority to test/, "Not supplied");
    await user.click(screen.getByRole("button", { name: "Add medication" }));
    await typeInto(user, /^Medication as written/, "Synthetic medication A 10 mg");
    await chooseWithKeyboard(user, /Reported by$/, "Ordering clinic");
    await user.click(screen.getByRole("button", { name: "Add point-of-care result" }));
    await typeInto(user, /^Device or test/, "Synthetic cup");
    await typeInto(user, /^Drug/, "Synthetic drug");
    await typeInto(user, /^Result as written/, " POS ");
    await typeInto(user, /^Reported by/, "Clinic nurse");
    await user.click(screen.getByRole("button", { name: "Save order" }));

    await screen.findByText(`Navigated to /orders/${ORDER_ID}`);
    const input = register.mock.calls[0]?.[0];
    expect(input).toMatchObject({
      input: {
        clientLocationId: CLIENTS[0]?.locations[0]?.id,
        sourceKind: "fax",
        billingResponsibility: "patient_self_pay",
        templateUse: {
          kind: "clinic_template",
          name: "Synthetic panel form",
          confirmedPerPatient: true,
        },
        items: [
          {
            orderableVersionId: SCREEN.id,
            sequenceRole: "first_line",
            reflexAuthorization: "authorized",
            reflexScopeOrderableIds: [ORDERABLES[1]?.orderableId],
          },
        ],
        evidence: {
          authority: { kind: "not_supplied", documentRef: null },
          medications: [
            {
              receivedText: "Synthetic medication A 10 mg",
              source: "ordering_clinic",
              asOfDate: null,
            },
          ],
          // The result text is kept exactly as written, spaces included.
          pointOfCareResults: [{ resultText: " POS ", source: "Clinic nurse" }],
        },
      },
    });
  });

  it("shows the engine's refusal and keeps what was entered", async () => {
    const { user } = renderPage({
      register_order: () =>
        Promise.reject(
          engineError("validation", "Check the highlighted fields.", {
            fieldErrors: [
              { field: "placerOrderId", code: "duplicate", message: "This placer order exists." },
            ],
          }),
        ),
    });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await pickRecord(user, /^Client/, "Synthetic");
    await pickRecord(user, /^Ordering practitioner/, "Doctor");
    await chooseWithKeyboard(user, /Received as/, "Fax");
    await user.click(screen.getByText("Clinic order number and source references"));
    await typeInto(user, /^Clinic order number$/, "PO-0001");
    await chooseWithKeyboard(user, /Billing responsibility/, "Client (ordering clinic)");
    await chooseWithKeyboard(user, /Template use/, "No template");
    await user.click(screen.getByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Not authorized");
    await chooseWithKeyboard(user, /Authority to test/, "Electronic signature");
    await user.click(screen.getByRole("button", { name: "Save order" }));
    expect(await screen.findByText("This placer order exists.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /^Clinic order number$/ })).toHaveProperty(
      "value",
      "PO-0001",
    );
  });

  it("puts the engine's nested field errors on the fields they name", async () => {
    const { user } = renderPage({
      register_order: () =>
        Promise.reject(
          engineError("validation", "Check the highlighted fields.", {
            fieldErrors: [
              {
                field: "evidence.diagnoses[0].codeSystem",
                code: "invalid",
                message: "Unknown system.",
              },
              { field: "templateUse.name", code: "too_long", message: "Shorten the name." },
              {
                field: "items[0].reflexScopeOrderableIds",
                code: "invalid",
                message: "This test cannot be reflexed.",
              },
            ],
          }),
        ),
    });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await pickRecord(user, /^Client/, "Synthetic");
    await pickRecord(user, /^Ordering practitioner/, "Doctor");
    await chooseWithKeyboard(user, /Received as/, "Fax");
    await chooseWithKeyboard(user, /Billing responsibility/, "Not supplied");
    await chooseWithKeyboard(user, /Template use/, "Clinic template");
    await typeInto(user, /^Template name/, "Synthetic panel form");
    await chooseWithKeyboard(user, /Confirmed for this patient/, "Yes");
    await user.click(screen.getByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Authorized");
    const scope = screen.getByRole("group", { name: /Reflex tests authorized/ });
    await user.click(within(scope).getByRole("checkbox", { name: /SYN-CNF/ }));
    await chooseWithKeyboard(user, /Authority to test/, "Not supplied");
    await user.click(screen.getByRole("button", { name: "Add diagnosis" }));
    await typeInto(user, /^Code\b(?! system)/, "Z00.00");
    await typeInto(user, /^Code system\b(?! version)/, "SYN-CODES");
    await typeInto(user, "Source document", "REQ-SCAN-0001");
    await chooseWithKeyboard(user, /Pre-printed on the laboratory's form/, "No");
    await user.click(screen.getByRole("button", { name: "Save order" }));

    expect(
      await screen.findByRole("textbox", {
        name: /^Code system\b(?! version)/,
        description: /Unknown system\./,
      }),
    ).toBeTruthy();
    expect(
      screen.getByRole("textbox", { name: /^Template name/, description: /Shorten the name\./ }),
    ).toBeTruthy();
    expect(
      screen.getByRole("group", {
        name: /Reflex tests authorized/,
        description: /This test cannot be reflexed\./,
      }),
    ).toBeTruthy();
  });

  it("shows who the order is for, and starts with every choice empty (REQ-ORD-005)", async () => {
    renderPage();
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    expect(screen.getByRole("combobox", { name: /^Client/ })).toHaveProperty("value", "");
    expect(screen.getByRole("combobox", { name: /^Ordering practitioner/ })).toHaveProperty(
      "value",
      "",
    );
    expect(screen.getByRole("button", { name: /Client location/ }).textContent).toContain(
      "Select…",
    );
    for (const box of screen.getAllByRole("checkbox")) expect(box).toHaveProperty("checked", false);
    // A short catalog is a plain checklist.
    expect(screen.queryByRole("searchbox", { name: "Find tests" })).toBeNull();
  });

  it("lists only the chosen client's locations, and clears the location when the client changes", async () => {
    const { user } = renderPage({ list_clients: () => TWO_CLIENTS });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    // Nothing to choose before the client is chosen.
    expect(screen.getByRole("button", { name: /Client location/ })).toHaveProperty(
      "disabled",
      true,
    );
    await pickRecord(user, /^Client/, "syn-clinic");
    await chooseWithKeyboard(user, /Client location/, "North Office");
    await pickRecord(user, /^Client/, "{Control>}a{/Control}syn-south");
    const location = screen.getByRole("button", { name: /Client location/ });
    expect(location.textContent).toContain("Select…");
    await user.click(location);
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual(["South Office"]);
  });

  it("adds tests by keyboard without moving focus and keeps their fields when another is removed", async () => {
    const { user, container } = renderPage({ list_active_orderables: () => LONG_CATALOG });
    const find = await screen.findByRole("combobox", { name: /Find tests/ });
    const chosen = screen.getByRole("list", { name: "Chosen tests" });
    expect(within(chosen).getByText("No tests chosen yet.")).toBeTruthy();
    await user.type(find, "SYN-SCR");
    await user.keyboard("{ArrowDown}{Enter}");
    expect(document.activeElement).toBe(find);
    expect(find).toHaveProperty("value", "");
    await user.type(find, "SYN-CNF");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    expect(screen.getByText("2 selected")).toBeTruthy();
    await chooseWithKeyboard(user, /Testing sequence role for SYN-CNF/, "Confirmation");
    await chooseWithKeyboard(user, /Reflex testing for SYN-CNF/, "Not authorized");
    await user.click(
      within(chosen).getByRole("button", { name: "Remove SYN-SCR Synthetic drug screen" }),
    );
    expect(
      screen.getByRole("button", { name: /Testing sequence role for SYN-CNF/ }).textContent,
    ).toContain("Confirmation");
    expect(
      screen.getByRole("button", { name: /Reflex testing for SYN-CNF/ }).textContent,
    ).toContain("Not authorized");
    expect(screen.queryByRole("group", { name: /SYN-SCR Synthetic drug screen$/ })).toBeNull();
    await user.type(find, "zzz");
    expect(screen.getByRole("listbox").textContent).toBe("No matches");
    await user.keyboard("{Enter}");
    expect(screen.queryByText("Choose client.")).toBeNull();
    await user.keyboard("{Escape}");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("filters panels without losing chosen tests and searches specimen codes", async () => {
    const catalog = LONG_CATALOG.map((item, index) =>
      index < 2 ? item : { ...item, kind: "panel" },
    );
    const { user } = renderPage({ list_active_orderables: () => catalog });
    const find = await screen.findByRole("combobox", { name: /Find tests/ });
    await user.type(find, "SYN-SCR");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    await chooseWithKeyboard(user, /Tests and panels Show/, "Panels only");
    await user.type(find, "urine");
    const options = within(screen.getByRole("listbox")).getAllByRole("option");
    expect(options.every((option) => option.textContent?.includes("Panel"))).toBe(true);
    expect(options.some((option) => option.textContent?.includes("SYN-SCR"))).toBe(false);
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    expect(screen.getByText("2 selected")).toBeTruthy();
    expect(screen.getByRole("group", { name: /SYN-SCR Synthetic drug screen$/ })).toBeTruthy();
    expect(screen.getByRole("group", { name: /SYN-T10 Synthetic panel 10$/ })).toBeTruthy();
  });

  // jsdom re-renders the 1,000-row list on each keystroke: about 20 s on a developer computer
  // and close to 40 s on a loaded CI runner, beyond the walkthrough limit.
  it(
    "finds tests across a 1,000-test catalog and uses the same keyboard picker for reflex scope",
    async () => {
      const { user } = renderPage({ list_active_orderables: () => LARGE_CATALOG });
      const find = await screen.findByRole("combobox", { name: /Find tests/ });
      expect(screen.queryByRole("checkbox", { name: /SYN-SCR/ })).toBeNull();
      await user.type(find, "SYN-SCR");
      await user.keyboard("{ArrowDown}{Enter}{Escape}");
      await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Authorized");
      await user.type(
        screen.getByRole("combobox", {
          name: "Find reflex tests for SYN-SCR Synthetic drug screen",
        }),
        "SYN-CNF",
      );
      await user.keyboard("{ArrowDown}{Enter}{Escape}");
      const chosen = screen.getByRole("list", {
        name: "Chosen reflex tests for SYN-SCR Synthetic drug screen",
      });
      expect(within(chosen).getByText(/SYN-CNF/)).toBeTruthy();
    },
    3 * WALKTHROUGH_TIMEOUT_MS,
  );

  it("asks before leaving typed input", async () => {
    const { user } = renderPage();
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await user.click(screen.getByText("Clinic order number and source references"));
    await typeInto(user, /^Clinic order number$/, "PO-0001");
    await user.click(await screen.findByRole("link", { name: "Patient, Test" }));
    const leave = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(leave).getByRole("button", { name: "Leave without saving" }));
    expect(await screen.findByText(`Navigated to /patients/${SUBJECT_ID}`)).toBeTruthy();
  });

  it("asks for the patient first when none was chosen", async () => {
    const { user } = renderPage({}, "/orders/new");
    expect(await screen.findByRole("heading", { name: "Choose patient" })).toBeTruthy();
    await openRowWithEnter(user, "MRN-00001 (SYN-CLINIC MR)");
    expect(await screen.findByRole("form", { name: "New order" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "Patient identity" })).toBeTruthy();
    expect(screen.queryByText(/Navigated to \/patients/)).toBeNull();
  });

  it("explains when the catalog has nothing to order", async () => {
    renderPage({ list_active_orderables: () => [] });
    expect(await screen.findByText("No tests can be ordered")).toBeTruthy();
  });

  it("shows a reference list failure with a retry", async () => {
    renderPage({
      list_clients: () => Promise.reject(engineError("internal", "The app could not load this.")),
    });
    expect(await screen.findByText("The app could not load this.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });
  it("saves once before the explicit review-and-receive checkpoint", async () => {
    const register = vi.fn(() => ORDER);
    const accept = vi.fn();
    const { user } = renderPage({ register_order: register, accept_order: accept });
    await screen.findByRole("form", { name: "New order" });
    await pickRecord(user, /^Client/, "syn-clinic");
    await pickRecord(user, /^Ordering practitioner/, "0000000000");
    await chooseWithKeyboard(user, /Received as/, "Paper requisition");
    await chooseWithKeyboard(user, /Billing responsibility/, "Not supplied");
    await chooseWithKeyboard(user, /Template use/, "No template");
    await user.click(screen.getByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Testing sequence role for SYN-SCR/, "First-line test");
    await chooseWithKeyboard(user, /Reflex testing for SYN-SCR/, "Not stated");
    await chooseWithKeyboard(user, /Authority to test/, "Handwritten signature");
    await user.click(screen.getByRole("button", { name: "Review and receive specimens" }));
    expect(await screen.findByText(`Navigated to /orders/${ORDER_ID}?next=receive`)).toBeTruthy();
    expect(register).toHaveBeenCalledTimes(1);
    expect(accept).not.toHaveBeenCalled();
  });

  it("keeps patient registration inside the new-order task", async () => {
    const { user } = renderPage({}, "/orders/new");
    await user.click(await screen.findByRole("button", { name: "Register patient" }));
    expect(
      await screen.findByText("Navigated to /patients/register?intent=new-order"),
    ).toBeTruthy();
  });

  it("protects entered order details when changing the patient", async () => {
    const { user } = renderPage();
    await screen.findByRole("form", { name: "New order" });
    await user.click(screen.getByText("Clinic order number and source references"));
    await typeInto(user, /^Clinic order number$/, "SOURCE-42");
    await user.click(screen.getByRole("button", { name: "Change patient" }));
    const leave = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(leave).getByRole("button", { name: "Leave without saving" }));
    expect(await screen.findByRole("region", { name: "Choose patient" })).toBeTruthy();
  });
});
