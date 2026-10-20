import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, typeInto, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderRoutes } from "@/lib/test/router";
import {
  ACCESSION,
  ACCESSION_CONTEXT,
  ACCESSION_ID,
  ITEM_ID,
  ORDER,
  ORDER_ID,
  orderAccessionContext,
  SUBJECT_VIEW,
  sessionWith,
} from "../testData";
import { NewAccessionPage } from "./NewAccessionPage";

const ACCEPTED_ORDER = {
  ...ORDER,
  status: "accepted",
  items: ORDER.items.map((item) => ({ ...item, state: "accepted" })),
};

const SPECIMEN_TYPES = [
  { id: "st-1", code: "urine", name: "Urine", versionInEffect: null, draftVersion: null },
];

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    // The patient's name in the banner links to the patient for people who enter orders.
    current_session: () => sessionWith(["accession", "enter_orders"]),
    get_order: () => ACCEPTED_ORDER,
    get_subject: () => SUBJECT_VIEW,
    list_specimen_types: () => SPECIMEN_TYPES,
    get_order_accession_context: () => ACCESSION_CONTEXT,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/orders/:orderId/accession", element: <NewAccessionPage /> }],
    `/orders/${ORDER_ID}/accession`,
  );
}

describe("NewAccessionPage (REQ-SPC-001, REQ-SPC-002)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("accessions an accepted order with the keyboard only, time and offset as recorded", async () => {
    const create = vi.fn(() => ACCESSION);
    const { user, container } = renderPage({ create_accession: create });
    expect(screen.getByRole("heading", { level: 1, name: "Receive specimens" })).toBeTruthy();
    const test = await screen.findByRole("checkbox", { name: /SYN-SCR Synthetic drug screen/ });
    expect(screen.getByText("Specimen types: urine")).toBeTruthy();
    // Who the material is for, under the header.
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await tabTo(user, test);
    await user.keyboard(" ");
    await chooseWithKeyboard(user, /Collection time known$/, "Date and time (UTC offset supplied)");
    await typeInto(user, /^Collection date/, "2026-10-12");
    await typeInto(user, /^Collection time$/, "08:30");
    await typeInto(user, /^Collection timezone/, "-05:00");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "2");
    await typeInto(user, /^Volume\b(?! unit)/, "30.0");
    await typeInto(user, /^Volume unit/, "mL");
    await chooseWithKeyboard(user, /Volume is$/, "Estimated");
    await tabTo(user, screen.getByRole("button", { name: "Receive specimens" }));
    await user.keyboard("{Enter}");

    expect(await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`)).toBeTruthy();
    expect(screen.getByText("Accession A26000001 created")).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: {
        orderId: ORDER_ID,
        orderItemIds: [ITEM_ID],
        collection: {
          time: { kind: "datetime", local: "2026-10-12T08:30", utcOffsetMinutes: -300 },
          observations: [],
        },
        specimens: [
          {
            specimenTypeCode: "urine",
            containerCount: 2,
            volume: "30.0",
            volumeUnit: "mL",
            volumeKind: "estimated",
          },
        ],
      },
    });
  });

  it("records an unknown collection time as unknown, never a guess", async () => {
    const create = vi.fn(() => ACCESSION);
    const { user } = renderPage({ create_accession: create });
    await user.click(await screen.findByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
    expect(screen.queryByRole("textbox", { name: /^Collection date/ })).toBeNull();
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`);
    expect(create).toHaveBeenCalledWith({
      input: expect.objectContaining({
        // Fields left empty are left out of the input, not sent as "".
        collection: { time: { kind: "unknown" }, observations: [] },
        specimens: [
          {
            specimenTypeCode: "urine",
            containerCount: 1,
            volumeKind: "unknown",
          },
        ],
      }),
    });
  });

  it("opens the created accession after eligibility refresh removes its tests", async () => {
    let saved = false;
    const { user } = renderPage({
      get_order_accession_context: () =>
        saved
          ? orderAccessionContext({
              items: ACCESSION_CONTEXT.items.map((item) => ({
                ...item,
                eligible: false,
                reason: "Already accessioned",
                accessionId: ACCESSION_ID,
                accessionNumber: "A26000001",
              })),
              allowedActions: [],
            })
          : ACCESSION_CONTEXT,
      create_accession: () => {
        saved = true;
        return ACCESSION;
      },
    });
    await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
    await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`)).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it.each(["get_order", "get_order_accession_context"])(
    "opens the saved accession even if the %s refresh fails",
    async (command) => {
      let saved = false;
      let finishRefresh: (() => void) | undefined;
      const otherCommand = command === "get_order" ? "get_order_accession_context" : "get_order";
      const otherData = command === "get_order" ? ACCESSION_CONTEXT : ACCEPTED_ORDER;
      const create = vi.fn(() => {
        saved = true;
        return ACCESSION;
      });
      const { user } = renderPage({
        [command]: () =>
          saved
            ? Promise.reject(engineError("internal", "The saved order could not be refreshed."))
            : command === "get_order"
              ? ACCEPTED_ORDER
              : ACCESSION_CONTEXT,
        [otherCommand]: () =>
          saved
            ? new Promise((resolve) => {
                finishRefresh = () => resolve(otherData);
              })
            : otherData,
        create_accession: create,
      });
      await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
      await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
      await chooseWithKeyboard(user, /Specimen type$/, "Urine");
      await typeInto(user, /^Containers/, "1");
      await chooseWithKeyboard(user, /Volume is$/, "Unknown");
      await user.click(screen.getByRole("button", { name: "Receive specimens" }));
      await screen.findByText("The saved order could not be refreshed.");
      expect(screen.getByRole("textbox", { name: /^Containers/ })).toHaveProperty("value", "1");
      if (!finishRefresh) throw new Error("The other invalidation did not start");
      finishRefresh();
      expect(await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`)).toBeTruthy();
      expect(create).toHaveBeenCalledTimes(1);
      expect(screen.queryByRole("alertdialog")).toBeNull();
    },
  );

  it("preserves a date-only collection without adding a time or timezone", async () => {
    const create = vi.fn(() => ACCESSION);
    const { user } = renderPage({ create_accession: create });
    await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
    await chooseWithKeyboard(user, /Collection time known$/, "Date only; time not recorded");
    await typeInto(user, /^Collection date/, "2026-10-12");
    expect(screen.queryByRole("textbox", { name: /^Collection time/ })).toBeNull();
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`);
    expect(create).toHaveBeenCalledWith({
      input: expect.objectContaining({
        collection: { time: { kind: "date", date: "2026-10-12" }, observations: [] },
      }),
    });
  });

  it("checks presence and format before creating", async () => {
    const create = vi.fn();
    const { user } = renderPage({ create_accession: create });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    await chooseWithKeyboard(user, /Collection time known$/, "Date only; time not recorded");
    await typeInto(user, /^Collection date/, "12/10/2026");
    await typeInto(user, /^Containers/, "0");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText("Choose at least one test.")).toBeTruthy();
    expect(screen.getByText("Enter the date as YYYY-MM-DD.")).toBeTruthy();
    expect(screen.getByText("Enter a whole number of 1 or more.")).toBeTruthy();
    expect(screen.getByText("Choose specimen type.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();
  });

  it("shows the engine's refusal, such as a test on hold", async () => {
    const { user } = renderPage({
      create_accession: () =>
        Promise.reject(engineError("gate_blocked", "A test on this order has an open hold.")),
    });
    await user.click(await screen.findByRole("checkbox", { name: /SYN-SCR/ }));
    await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText("A test on this order has an open hold.")).toBeTruthy();
    expect(screen.getByText(/corr-test-0001/)).toBeTruthy();
    expect(screen.getByRole("checkbox", { name: /SYN-SCR/ })).toHaveProperty("checked", true);
    expect(screen.getByRole("textbox", { name: /^Containers/ })).toHaveProperty("value", "1");
  });

  it("asks before leaving typed input, by Cancel or by a link", async () => {
    const { user } = renderPage();
    await user.click(await screen.findByRole("checkbox", { name: /SYN-SCR/ }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    const stay = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(stay).getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("checkbox", { name: /SYN-SCR/ })).toHaveProperty("checked", true);

    await user.click(await screen.findByRole("link", { name: "Patient, Test" }));
    const leave = await screen.findByRole("alertdialog", { name: "Leave without saving?" });
    await user.click(within(leave).getByRole("button", { name: "Leave without saving" }));
    expect(await screen.findByText(/Navigated to \/patients\//)).toBeTruthy();
  });

  it("offers only engine-eligible tests and preserves the exact selected scope", async () => {
    const original = ORDER.items[0];
    const itemContext = ACCESSION_CONTEXT.items[0];
    if (!original || !itemContext) throw new Error("Missing fixture");
    const create = vi.fn(() => ACCESSION);
    const { user } = renderPage({
      get_order: () => ({
        ...ACCEPTED_ORDER,
        items: [
          original,
          {
            ...original,
            itemId: "eligible-2",
            orderableCode: "OF",
            orderableName: "Oral fluid panel",
          },
          {
            ...original,
            itemId: "allocated",
            orderableCode: "OLD",
            orderableName: "Previously accessioned test",
          },
          { ...original, itemId: "held", orderableCode: "HOLD", orderableName: "Held test" },
        ],
      }),
      get_order_accession_context: () =>
        orderAccessionContext({
          items: [
            itemContext,
            {
              ...itemContext,
              orderItemId: "eligible-2",
              requiredSpecimenTypeCodes: ["oral_fluid"],
            },
            {
              ...itemContext,
              orderItemId: "allocated",
              eligible: false,
              reason: "Already assigned to an accession.",
              accessionId: ACCESSION_ID,
              accessionNumber: "A26000001",
            },
            {
              ...itemContext,
              orderItemId: "held",
              eligible: false,
              reason: "Order authority is on hold.",
            },
          ],
        }),
      create_accession: create,
    });
    await screen.findByRole("checkbox", { name: /SYN-SCR/ });
    expect(screen.queryByRole("checkbox", { name: /Previously accessioned|Held test/ })).toBeNull();
    expect(screen.getByText(/Order authority is on hold/)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Accession A26000001" })).toHaveProperty(
      "href",
      expect.stringContaining(`/accessions/${ACCESSION_ID}`),
    );
    await user.click(screen.getByRole("button", { name: "Use all eligible tests" }));
    expect(screen.getByRole("checkbox", { name: /SYN-SCR/ })).toHaveProperty("checked", true);
    expect(screen.getByRole("checkbox", { name: /Oral fluid panel/ })).toHaveProperty(
      "checked",
      true,
    );
    expect(screen.getByRole("status").textContent).toContain("urine, oral_fluid");
    await user.click(screen.getByRole("checkbox", { name: /Oral fluid panel/ }));
    expect(screen.getByRole("status").textContent).not.toContain("oral_fluid");
    await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`);
    expect(create).toHaveBeenCalledWith({
      input: expect.objectContaining({ orderItemIds: [ITEM_ID] }),
    });
  });

  it("clears selected tests without discarding entered material", async () => {
    const { user } = renderPage();
    await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
    await typeInto(user, /^Containers/, "2");
    await user.click(screen.getByRole("button", { name: "Clear selected tests" }));
    expect(screen.getByRole("checkbox", { name: /SYN-SCR/ })).toHaveProperty("checked", false);
    expect(screen.getByRole("textbox", { name: /^Containers/ })).toHaveProperty("value", "2");
    expect(screen.queryByText(/Required specimen types for the selected tests/)).toBeNull();
    expect(screen.getByText("Choose at least one test.")).toBeTruthy();
  });

  it("searches a long eligible list and saves only the selected tests", async () => {
    const create = vi.fn(() => ACCESSION);
    const { user, container } = renderPage({
      get_order: () => ({
        ...ACCEPTED_ORDER,
        items: ACCEPTED_ORDER.items.flatMap((item) =>
          Array.from({ length: 20 }, (_, index) => ({
            ...item,
            itemId: `item-${index}`,
            orderableCode: `SYN-${index}`,
            orderableName: `Synthetic test ${index}`,
          })),
        ),
      }),
      get_order_accession_context: () =>
        orderAccessionContext({
          items: ACCESSION_CONTEXT.items.flatMap((item) =>
            Array.from({ length: 20 }, (_, index) => ({
              ...item,
              orderItemId: `item-${index}`,
            })),
          ),
        }),
      create_accession: create,
    });
    const picker = await screen.findByRole("combobox", { name: "Find ordered tests" });
    await expectNoA11yViolations(container, { wholePage: true });
    await tabTo(user, picker);
    await user.keyboard("SYN-17{ArrowDown}{Enter}{Escape}");
    expect(
      screen.getByRole("list", { name: "Selected tests for this material" }).textContent,
    ).toContain("SYN-17 Synthetic test 17");
    await user.clear(picker);
    await user.type(picker, "no-matching-test");
    expect(screen.getByRole("listbox").textContent).toBe("No matches");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Remove SYN-17 Synthetic test 17" }));
    expect(picker).toBe(document.activeElement);
    expect(screen.queryByRole("list", { name: "Selected tests for this material" })).toBeNull();
    await user.clear(picker);
    await user.type(picker, "SYN-18");
    await user.keyboard("{ArrowDown}{Enter}{Escape}");
    await chooseWithKeyboard(user, /Collection time known$/, "Unknown");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`);
    expect(create).toHaveBeenCalledWith({
      input: expect.objectContaining({ orderItemIds: ["item-18"] }),
    });
  });

  it("continues an existing accession when no eligible work remains", async () => {
    const first = ACCESSION_CONTEXT.items[0];
    if (!first) throw new Error("Missing fixture");
    renderPage({
      get_order_accession_context: () =>
        orderAccessionContext({
          items: [
            {
              ...first,
              eligible: false,
              reason: "Already assigned to an accession.",
              accessionId: ACCESSION_ID,
              accessionNumber: "A26000001",
            },
          ],
          accessions: [
            {
              accessionId: ACCESSION_ID,
              accessionNumber: "A26000001",
              createdAt: ACCESSION.createdAt,
              specimenCount: 1,
              pendingReceiptCount: 1,
            },
          ],
          allowedActions: [],
        }),
    });
    expect(await screen.findByText("No tests available to receive")).toBeTruthy();
    expect(screen.queryByRole("form", { name: "Receive specimens" })).toBeNull();
    const existing = screen.getByRole("list", { name: "Existing accessions" });
    expect(within(existing).getByRole("link", { name: "Accession A26000001" })).toBeTruthy();
    expect(existing.textContent).toContain("1 need receipt review");
  });

  it("sends a named collection timezone without guessing the UTC offset", async () => {
    const create = vi.fn(() => ACCESSION);
    const { user } = renderPage({ create_accession: create });
    await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
    await chooseWithKeyboard(user, /Collection time known$/, "Date and time (named timezone)");
    expect(screen.getByRole("textbox", { name: /^Collection site timezone/ })).toHaveProperty(
      "value",
      "",
    );
    await typeInto(user, /^Collection date/, "2026-10-12");
    await typeInto(user, /^Collection time$/, "08:30");
    await typeInto(user, /^Collection site timezone/, "America/New_York");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    await screen.findByText(`Navigated to /accessions/${ACCESSION_ID}`);
    expect(create).toHaveBeenCalledWith({
      input: expect.objectContaining({
        collection: {
          time: { kind: "zoned", local: "2026-10-12T08:30", timeZone: "America/New_York" },
          observations: [],
        },
      }),
    });
  });

  it("maps an engine timezone refusal to the field and preserves entered material", async () => {
    const message = "Enter a collection-site timezone such as America/New_York.";
    const { user } = renderPage({
      create_accession: () =>
        Promise.reject(
          engineError("validation", "Check the collection timezone.", {
            fieldErrors: [{ field: "collection.time.timeZone", code: "invalid_zone", message }],
          }),
        ),
    });
    await user.click(await screen.findByRole("button", { name: "Use all eligible tests" }));
    await chooseWithKeyboard(user, /Collection time known$/, "Date and time (named timezone)");
    await typeInto(user, /^Collection date/, "2026-10-12");
    await typeInto(user, /^Collection time$/, "08:30");
    await typeInto(user, /^Collection site timezone/, "Unknown/Zone");
    await chooseWithKeyboard(user, /Specimen type$/, "Urine");
    await typeInto(user, /^Containers/, "1");
    await chooseWithKeyboard(user, /Volume is$/, "Unknown");
    await user.click(screen.getByRole("button", { name: "Receive specimens" }));
    expect(await screen.findByText(message)).toBeTruthy();
    const field = screen.getByRole("textbox", { name: /^Collection site timezone/ });
    expect(field.getAttribute("aria-invalid")).toBe("true");
    expect(field).toHaveProperty("value", "Unknown/Zone");
    expect(screen.getByRole("textbox", { name: /^Containers/ })).toHaveProperty("value", "1");
  });

  it("shows a load failure with a retry", async () => {
    renderPage({ get_order: () => Promise.reject(engineError("not_found", "No such order.")) });
    expect(await screen.findByText("No such order.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("explains when no specimen types are set up", async () => {
    renderPage({ list_specimen_types: () => [] });
    expect(await screen.findByText("No specimen types are set up")).toBeTruthy();
  });
});
