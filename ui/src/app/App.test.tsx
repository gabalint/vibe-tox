import { screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { LAB_PROFILE, LISTENER, renderApp } from "./frameTestData";
import { routes } from "./routes";

describe("App frame", () => {
  it("lands on the work queues after sign-in, with the top bar and the navigation", async () => {
    const { container } = renderApp("/", ["perform_analysis"]);
    expect(await screen.findByRole("heading", { level: 1, name: "Work queues" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Work queues" }).getAttribute("aria-current")).toBe(
      "page",
    );
    expect(screen.getByRole("navigation", { name: "Main" })).toBeTruthy();
    expect(await screen.findByRole("button", { name: "No instruments configured" })).toBeTruthy();
    expect(await screen.findByRole("link", { name: "Health: OK" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Signed in as Test Analyst" })).toBeTruthy();
    await waitFor(() => expect(document.title).toBe("Work queues · Vibe Tox LIS"));
    await expectNoA11yViolations(container, { wholePage: true });
  });

  // WCAG 2.2 SC 2.4.2 and 2.4.3: the title follows the page and focus moves to its heading.
  it("titles the window after the page and focuses the page heading on navigation", async () => {
    const { user } = renderApp("/about", ["perform_analysis"]);
    const about = await screen.findByRole("heading", { level: 1, name: "About this app" });
    await waitFor(() => expect(document.title).toBe("About this app · Vibe Tox LIS"));
    // The first screen leaves focus where it was.
    expect(document.activeElement).not.toBe(about);
    await user.click(screen.getByRole("link", { name: "Work queues" }));
    const heading = await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await waitFor(() => expect(document.activeElement).toBe(heading));
    expect(document.title).toBe("Work queues · Vibe Tox LIS");
  });

  it("skips from the top of the page to the main content", async () => {
    const { user } = renderApp("/", ["perform_analysis"]);
    await screen.findByRole("heading", { level: 1, name: "Work queues" });
    await user.tab();
    const skip = screen.getByRole("button", { name: "Skip to main content" });
    expect(document.activeElement).toBe(skip);
    await user.keyboard("{Enter}");
    expect(document.activeElement).toBe(screen.getByRole("main"));
  });

  // REQ-SPC-001, ui-standards §Forms: a scanner types the barcode and presses Enter.
  it("sends a scanned barcode from the top bar to the container lookup", async () => {
    const { user, router } = renderApp("/", ["accession"]);
    const field = await screen.findByRole("searchbox", { name: "Scan or search" });
    await user.type(field, "  C00001234 {Enter}");
    await waitFor(() =>
      expect(`${router.state.location.pathname}${router.state.location.search}`).toBe(
        "/containers?labelCode=C00001234",
      ),
    );
    expect(await screen.findByRole("heading", { level: 1, name: "Container lookup" })).toBeTruthy();
    expect((field as HTMLInputElement).value).toBe("");
  });

  it("ignores an empty scan, and shows no scan field without the accession capability", async () => {
    const { user, router } = renderApp("/", ["accession"]);
    const field = await screen.findByRole("searchbox", { name: "Scan or search" });
    await user.type(field, "   {Enter}");
    expect(router.state.location.pathname).toBe("/work-queues");
    renderApp("/", ["perform_analysis"]);
    await screen.findAllByRole("heading", { level: 1, name: "Work queues" });
    expect(screen.getAllByRole("searchbox", { name: "Scan or search" })).toHaveLength(1);
  });

  // REQ-OPS-013: one summary in the top bar; every listener behind it.
  it("summarizes the analyzer listeners and shows each one on request", async () => {
    const { user } = renderApp("/", ["perform_analysis"], {
      list_listener_status: () => [
        LISTENER,
        {
          ...LISTENER,
          instrumentId: "00000000-0000-7000-8000-0000000000a2",
          instrumentName: "Synthetic Analyzer 2",
          state: "error",
          lastActivityAt: null,
          detail: "open_failed",
        },
      ],
    });
    const summary = await screen.findByRole("button", {
      name: "2 analyzers: 1 error, 1 receiving",
    });
    expect(screen.queryByText("Synthetic Analyzer 1")).toBeNull();
    await user.click(summary);
    const dialog = await screen.findByRole("dialog", { name: "Analyzer listeners" });
    expect(within(dialog).getByText("Synthetic Analyzer 1")).toBeTruthy();
    expect(within(dialog).getByText("Could not open the connection")).toBeTruthy();
    await expectNoA11yViolations(dialog);
    await user.click(within(dialog).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  // REQ-RPT-001: local times follow the laboratory profile's time zone, named.
  it("shows times in the laboratory time zone once a profile exists", async () => {
    const { user } = renderApp("/", ["perform_analysis"], {
      get_laboratory_profile: () => ({ ...LAB_PROFILE, timeZone: "America/Chicago" }),
      list_listener_status: () => [LISTENER],
    });
    await user.click(await screen.findByRole("button", { name: "1 analyzer: 1 receiving" }));
    expect(await screen.findByText("2026-10-12 09:03 CDT")).toBeTruthy();
    expect(screen.queryByText(/Times show in UTC/)).toBeNull();
  });

  it("shows times in UTC, and says so, until the laboratory profile is set", async () => {
    const { user } = renderApp("/", ["perform_analysis"], {
      get_laboratory_profile: () => null,
      list_listener_status: () => [LISTENER],
    });
    expect(
      await screen.findByText(
        "Times show in UTC until the laboratory profile and its time zone are set.",
      ),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "1 analyzer: 1 receiving" }));
    expect(await screen.findByText("2026-10-12 14:03 UTC")).toBeTruthy();
  });

  // REQ-OPS-013: listener status is visible before anyone signs in.
  it("shows the sign-in screen with every listener to a signed-out user on any route", async () => {
    mockCommands({
      current_session: () => null,
      setup_status: () => ({ needsAdministrator: false }),
      list_listener_status: () => [LISTENER],
    });
    renderWithProviders(
      <RouterProvider router={createMemoryRouter(routes, { initialEntries: ["/orders"] })} />,
    );
    expect(await screen.findByRole("heading", { level: 1, name: "Sign in" })).toBeTruthy();
    expect(await screen.findByText("Synthetic Analyzer 1")).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Main" })).toBeNull();
  });
});
