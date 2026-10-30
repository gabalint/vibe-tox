import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ImportAttemptSummary } from "@/generated/ImportAttemptSummary";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { MANAGER, sessionWith } from "../testConfiguration";
import { InstrumentsPage } from "./InstrumentsPage";

const AU_SERIAL: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001501",
  code: "SYN-AU",
  name: "Synthetic AU400",
  enabled: true,
  revision: 2,
  created: {
    by: "00000000-0000-7000-8000-000000000901",
    byDisplay: "Synthetic Configurer",
    at: "2026-10-20T08:00:00Z",
  },
  changed: {
    by: "00000000-0000-7000-8000-000000000902",
    byDisplay: "Synthetic Approver",
    at: "2026-10-28T09:30:00Z",
  },
  connection: {
    kind: "au_serial",
    portPath: "COM10",
    baudRate: 9600,
    dataBits: "eight",
    parity: "none",
    stopBits: "one",
    au: {
      protocol: {
        framing: { start: [2], end: [3], bcc: true, use_etb: false, max_wire_len: 512, class: "b" },
        format: {
          unit_no: false,
          rack_digits: 4,
          sample_id_digits: 20,
          sex: true,
          age: false,
          month: false,
          patient_info_digits: [4, 0, 0, 0, 0, 0],
          result_digits: 9,
        },
      },
      frameTimeoutMs: 2000,
      blockTimeoutMs: 3000,
    },
  },
};

const NETWORK: InstrumentConfiguration = {
  id: "00000000-0000-7000-8000-000000001502",
  code: "SYN-NET",
  name: "Synthetic analyzer",
  enabled: true,
  revision: 1,
  connection: {
    kind: "astm_tcp",
    bindAddress: "127.0.0.1",
    port: 5000,
    allowedPeers: ["127.0.0.1", "10.0.0.21"],
  },
};

const SOURCE = {
  registrationId: "00000000-0000-7000-8000-000000001511",
  instrumentId: AU_SERIAL.id,
  senderName: "SYN-AU-SW",
  senderVersion: "1.0",
  state: "active",
  testCodes: [
    { sourceTestCode: "001", analyteId: "a1" },
    { sourceTestCode: "002", analyteId: "a2" },
  ],
} as SourceRegistrationView;

const ATTEMPT = {
  importAttemptId: "00000000-0000-7000-8000-000000001521",
  instrumentId: AU_SERIAL.id,
  outcome: "processed_with_quarantine",
  processedAt: "2026-10-29T10:00:00Z",
} as ImportAttemptSummary;

function renderPage(capabilities: string[], commands: Record<string, () => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_instruments: () => [AU_SERIAL, NETWORK],
    list_listener_status: () => [
      {
        instrumentId: AU_SERIAL.id,
        instrumentName: AU_SERIAL.name,
        state: "connected",
        lastActivityAt: "2026-10-29T10:05:00Z",
        detail: null,
        lastError: "transport_failed",
        lastErrorAt: "2026-10-29T07:00:00Z",
      },
      {
        instrumentId: NETWORK.id,
        instrumentName: NETWORK.name,
        state: "error",
        lastActivityAt: "2026-10-29T09:00:00Z",
        detail: "open_failed",
        lastError: "open_failed",
        lastErrorAt: "2026-10-29T09:00:00Z",
      },
    ],
    list_instrument_readiness: () => [
      { instrumentId: AU_SERIAL.id, name: AU_SERIAL.name, status: "in_service", since: null },
    ],
    list_source_registrations: () => [],
    list_import_attempts: () => [],
    ...commands,
  });
  return renderRoutes([{ path: "/instruments", element: <InstrumentsPage /> }], "/instruments");
}

/** The value beside `label` in the panel's facts. */
function fact(panel: HTMLElement, label: string) {
  return within(panel).getByText(label, { selector: "dt" }).nextElementSibling;
}

describe("InstrumentPanel (REQ-INT-005)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows every setting read-only in the form's groups, with live state, source and imports", async () => {
    const { user, container } = renderPage([...MANAGER, "review_results"], {
      list_source_registrations: () => [SOURCE],
      list_import_attempts: () => [ATTEMPT, { ...ATTEMPT, outcome: "processed" }],
    });
    await user.click(await screen.findByRole("gridcell", { name: "SYN-AU" }));
    const panel = await screen.findByRole("region", { name: "SYN-AU" });
    expect([...panel.querySelectorAll("summary")].map((summary) => summary.textContent)).toEqual([
      "General",
      "Connection",
      "AU settings",
      "Live status",
      "Result source",
      "Activity",
      "Changes",
    ]);
    expect(fact(panel, "Connection type")?.textContent).toBe("AU400 / AU640 — serial port");
    expect(fact(panel, "Serial port")?.textContent).toBe("COM10");
    expect(fact(panel, "Parity")?.textContent).toBe("None");
    expect(fact(panel, "Transmission class")?.textContent).toBe("Class B — ACK / NAK");
    expect(fact(panel, "Start code (hex)")?.textContent).toBe("02");
    expect(fact(panel, "ETB block endings used")?.textContent).toBe("No");
    expect(fact(panel, "Rack number width")?.textContent).toBe("4 digits");
    expect(fact(panel, "Patient information 1–6 widths")?.textContent).toBe("4, 0, 0, 0, 0, 0");
    expect(await within(panel).findByText("In service")).toBeTruthy();
    expect(await within(panel).findByText("Connected")).toBeTruthy();
    expect(fact(panel, "Last error")?.textContent).toMatch(/^Connection failed, /);
    expect(fact(panel, "Registered")?.textContent).toMatch(/ by Synthetic Configurer$/);
    expect(fact(panel, "Last changed")?.textContent).toMatch(/ by Synthetic Approver$/);
    expect((await within(panel).findByText("SYN-AU-SW 1.0")).tagName).toBe("DD");
    expect(fact(panel, "Mapped test codes")?.textContent).toBe("2");
    expect(within(panel).getByText("Approved")).toBeTruthy();
    expect(
      within(panel).getByRole("link", { name: "Bench setup › Instruments" }).getAttribute("href"),
    ).toBe(`/bench/setup/instruments?open=${AU_SERIAL.id}`);
    const imports = await within(panel).findByRole("link", { name: "1 of 2" });
    expect(imports.getAttribute("href")).toBe(`/review/imports?instrument=${AU_SERIAL.id}`);
    expect(fact(panel, "Last import")?.querySelector("a")?.getAttribute("href")).toBe(
      `/review/imports/${ATTEMPT.importAttemptId}`,
    );
    expect(within(panel).getByRole("button", { name: "Edit connection" })).toBeTruthy();
    await expectNoA11yViolations(container);

    const au = within(panel).getByText("AU settings").closest("details");
    await user.click(within(panel).getByText("AU settings"));
    expect(au?.open).toBe(false);
  });

  it("shows a network listener's error, an unregistered source, and no imports without review rights", async () => {
    const { user } = renderPage(MANAGER);
    await user.click(await screen.findByRole("gridcell", { name: "SYN-NET" }));
    const panel = await screen.findByRole("region", { name: "SYN-NET" });
    expect(fact(panel, "Listen on address")?.textContent).toBe("127.0.0.1");
    expect(fact(panel, "Analyzer addresses allowed to connect")?.textContent).toBe(
      "127.0.0.1, 10.0.0.21",
    );
    expect(within(panel).queryByText("AU settings")).toBeNull();
    expect(fact(panel, "Last error")?.textContent).toMatch(/^Could not open the connection, /);
    expect(fact(panel, "Registered")?.textContent).toBe("Not recorded");
    expect(fact(panel, "Last changed")?.textContent).toBe("Not recorded");
    expect(await within(panel).findByText("Not registered")).toBeTruthy();
    expect(within(panel).getByText("Status not recorded")).toBeTruthy();
    expect(within(panel).queryByText("Activity")).toBeNull();
  });
});
