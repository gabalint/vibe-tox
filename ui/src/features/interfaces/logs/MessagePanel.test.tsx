import { act, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PartnerMessageDetail } from "@/generated/PartnerMessageDetail";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, CONNECTION, WRITTEN } from "../testData";
import { LogsPage } from "./LogsPage";

const ROUTES = [{ path: "/interfaces/logs/:tab?", element: <LogsPage /> }];
const DELIVERY = "01a1c02c-b86a-76b1-9fbe-fb18fc49e4aa";

/** A synthetic result sent over MLLP: no answer in time, then accepted. */
const SENT: PartnerMessageRow = {
  ...WRITTEN,
  deliveryId: DELIVERY,
  messageType: "hl7_oru",
  source: "report_revision",
  sourceNumber: 1,
  sourceLabel: "A26000009-UR",
  connection: { id: CONNECTION.id, name: "TEST MLLP Results", protocol: "mllp_connect" },
  outcomeUnknown: false,
  writtenPath: "127.0.0.1:2575",
  ackCode: "AA",
  ackText: "Message accepted",
};

const DETAIL: PartnerMessageDetail = {
  deliveryId: DELIVERY,
  controlId: DELIVERY,
  content: "sent",
  segments: [
    `MSH|^~\\&|VIBETOX LIS|LAB^00D0000000^CLIA||TEST-PARTNER|20261105100000-0500||ORU^R01^ORU_R01|${DELIVERY}|P|2.5.1`,
    "PID|1||TEST-00000^^^VIBETOX^MR||TEST^SYNTHETIC||19000101",
  ],
  attempts: [
    { at: "2026-11-05T10:00:00.000000000Z", succeeded: false, errorCode: "no_ack", ackCode: null },
    { at: "2026-11-05T10:01:00.000000000Z", succeeded: true, errorCode: null, ackCode: "AA" },
  ],
};

function renderPage(detail: (args: unknown) => unknown, rows = [SENT]) {
  mockCommands({
    current_session: () => ADMIN,
    list_partner_connections: () => [CONNECTION],
    list_partner_messages: () => ({ rows, more: false }),
    get_partner_message: detail,
  });
  return renderRoutes(ROUTES, "/interfaces/logs");
}

describe("Logs message details (ticket #4678)", () => {
  beforeEach(mockGridLayout);

  it("opens a sent message with its control ID, the message as sent, the ACK and the attempts", async () => {
    const detail = vi.fn(() => DETAIL);
    const { user, container } = renderPage(detail);
    // Nothing is read before the row is opened: the message holds patient data.
    await screen.findByRole("gridcell", { name: "A26000009-UR r1" });
    expect(detail).not.toHaveBeenCalled();
    await openRow(user, "A26000009-UR r1");
    const panel = await screen.findByRole("region", { name: /^A26000009-UR r1/ });
    expect(detail).toHaveBeenCalledWith({ input: { deliveryId: DELIVERY } });
    expect(await within(panel).findByText("Matches what was sent")).toBeTruthy();
    expect(within(panel).getAllByText(DELIVERY).length).toBeGreaterThan(0);
    const message = within(panel).getByText(/^MSH\|/);
    expect(message.textContent?.split("\n")).toEqual(DETAIL.segments);
    expect(within(panel).getByText("Accepted (AA)")).toBeTruthy();
    expect(within(panel).getByText("Message accepted")).toBeTruthy();
    expect(within(panel).getByText(/Failed: No acknowledgement in time/)).toBeTruthy();
    expect(within(panel).getByText(/Sent; the partner answered AA/)).toBeTruthy();
    const link = within(panel).getByRole("link", { name: "TEST MLLP Results" });
    await expectNoA11yViolations(container, { wholePage: true });
    act(() => link.focus());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: /^A26000009-UR/ })).toBeNull());
  });

  it("says when the kept copy cannot be shown, never showing another message", async () => {
    const { user } = renderPage(() => ({
      ...DETAIL,
      controlId: null,
      content: "unavailable",
      segments: [],
    }));
    await openRowWithEnter(user, "A26000009-UR r1");
    const panel = await screen.findByRole("region", { name: /^A26000009-UR r1/ });
    expect(await within(panel).findByText(/missing or no longer matches its digest/)).toBeTruthy();
    expect(within(panel).queryByText("Matches what was sent")).toBeNull();
    expect(within(panel).queryByText(/^MSH\|/)).toBeNull();
  });

  it("explains a file message and a message not made yet", async () => {
    const pdf = { ...SENT, messageType: "report_pdf" as const };
    const { user } = renderPage(
      () => ({ ...DETAIL, controlId: null, content: "file", segments: [], attempts: [] }),
      [pdf],
    );
    await openRow(user, "A26000009-UR r1");
    const panel = await screen.findByRole("region", { name: /^A26000009-UR r1/ });
    expect(await within(panel).findByText(/is a file, not an HL7 message/)).toBeTruthy();
    expect(within(panel).getByText("None yet.")).toBeTruthy();
  });
});
