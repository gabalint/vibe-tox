import { act, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { session } from "@/lib/test/session";
import { StarterBanner } from "./StarterBanner";
import { LOADED, NOT_LOADED } from "./testData";

const TITLE = "Starter catalog content is not yet reviewed: not ready for patient testing";

function renderBanner(status: () => unknown, capabilities: readonly string[]) {
  mockCommands({ current_session: () => session(capabilities), standard_catalog_status: status });
  return renderRoutes([{ path: "/", element: <StarterBanner /> }], "/");
}

describe("Starter catalog banner (proposed REQ-OPS-015)", () => {
  it("shows while loaded drafts await review, linking people who can act to Catalog review", async () => {
    const { user, container } = renderBanner(() => LOADED, ["approve_configuration"]);
    expect(await screen.findByText(TITLE)).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(await screen.findByRole("link", { name: "Catalog review" }));
    expect(await screen.findByText("Navigated to /configuration/catalog-review")).toBeTruthy();
  });

  it("shows every other signed-in person the notice without the link", async () => {
    renderBanner(() => LOADED, ["perform_analysis"]);
    expect(await screen.findByText(TITLE)).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Catalog review" })).toBeNull();
  });

  it("shows nothing once no loaded draft remains", async () => {
    const status = vi.fn(() => NOT_LOADED);
    renderBanner(status, ["perform_analysis"]);
    await waitFor(() => expect(status).toHaveBeenCalled());
    await act(() => Promise.resolve());
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows nothing when the status cannot be read", async () => {
    const status = vi.fn(() => Promise.reject(engineError("internal", "Failed.")));
    renderBanner(status, ["perform_analysis"]);
    await waitFor(() => expect(status).toHaveBeenCalled());
    await act(() => Promise.resolve());
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
