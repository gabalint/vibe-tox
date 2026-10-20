import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ContainerLookup } from "@/generated/ContainerLookup";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { ACCESSION_ID, container, SUMMARY, sessionWith } from "../testData";
import { ContainerLookupPage } from "./ContainerLookupPage";

const LOOKUP: ContainerLookup = {
  container: container(),
  specimenTypeCode: "urine",
  acceptance: "accepted",
  accessionId: ACCESSION_ID,
  accessionNumber: "A26000001",
  subject: SUMMARY,
};

function renderPage(findContainer: (args: unknown) => unknown, path = "/containers") {
  // The lookup returns the patient; no second read of the patient's record is made.
  mockCommands({
    find_container: findContainer,
    get_container_history: () => ({
      moves: [],
      holds: [],
      disposal: null,
      depletion: null,
      hasMore: false,
      snapshot: { maxMoveRowId: "0", maxHoldRowId: "0" },
    }),
    current_session: () => sessionWith(["accession", "enter_orders"]),
  });
  return renderRoutes([{ path: "/containers", element: <ContainerLookupPage /> }], path);
}

describe("ContainerLookupPage (REQ-SPC-001)", () => {
  it("shows the banner and the result of one scan without a duplicate React key", async () => {
    const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
    renderPage(() => LOOKUP, "/containers?labelCode=C00000001");
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "A26000001" })).toBeTruthy();
    const keyErrors = errors.mock.calls.filter((call) => String(call[0]).includes("same key"));
    errors.mockRestore();
    expect(keyErrors).toEqual([]);
  });

  it("finds a scanned barcode on Enter and clears the field for the next scan", async () => {
    const find = vi.fn(() => LOOKUP);
    const { user, container: page } = renderPage(find);
    await expectNoA11yViolations(page, { wholePage: true });

    // A scanner types into the focused field and presses Enter.
    await user.keyboard("C00000001{Enter}");
    expect(await screen.findByRole("link", { name: "A26000001" })).toBeTruthy();
    expect(find).toHaveBeenCalledWith({ input: { labelCode: "C00000001" } });
    expect(screen.getByText("Accepted")).toBeTruthy();
    expect(screen.getByText("30.0 mL, estimated")).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Patient, Test" })).toBeTruthy();
    const banner = screen.getByRole("region", { name: "Patient and specimen identity" });
    expect(within(banner).getByText("MRN-00001")).toBeTruthy();
    expect(within(banner).getByText("A26000001")).toBeTruthy();
    expect(within(banner).getByText("C00000001")).toBeTruthy();
    const field = screen.getByRole("textbox", { name: /Container barcode/ });
    expect(field).toHaveProperty("value", "");
    expect(document.activeElement).toBe(field);
    await expectNoA11yViolations(page, { wholePage: true });

    // Scanning the same container again looks it up again and shows its current state. The
    // first scan's result stays on screen until the new one renders, so wait for the second
    // answer itself, not for anything the first one already shows.
    find.mockReturnValueOnce({ ...LOOKUP, container: container({ state: "depleted" }) });
    await user.keyboard("C00000001{Enter}");
    expect(await screen.findByText("Depleted")).toBeTruthy();
    expect(find).toHaveBeenCalledTimes(2);
  });

  it("looks up the barcode the top bar's scan field sends in the address", async () => {
    const find = vi.fn(() => LOOKUP);
    renderPage(find, "/containers?labelCode=C00000001");
    expect(await screen.findByRole("link", { name: "A26000001" })).toBeTruthy();
    expect(find).toHaveBeenCalledWith({ input: { labelCode: "C00000001" } });
    expect(screen.getByRole("region", { name: "Patient and specimen identity" })).toBeTruthy();
  });

  it("says when no container has the barcode", async () => {
    const { user } = renderPage(() =>
      Promise.reject(engineError("not_found", "The container was not found.")),
    );
    await user.keyboard("C99999999{Enter}");
    expect(await screen.findByText("No container has the barcode C99999999")).toBeTruthy();
  });

  it("shows other engine errors with the support reference", async () => {
    const { user } = renderPage(() =>
      Promise.reject(engineError("forbidden", "You cannot look up containers.")),
    );
    await user.keyboard("C00000001{Enter}");
    expect(await screen.findByText("You cannot look up containers.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });

  it("requires a barcode before looking up", async () => {
    const find = vi.fn();
    const { user } = renderPage(find);
    await user.keyboard("{Enter}");
    expect(await screen.findByText("Enter container barcode.")).toBeTruthy();
    expect(find).not.toHaveBeenCalled();
  });
});
