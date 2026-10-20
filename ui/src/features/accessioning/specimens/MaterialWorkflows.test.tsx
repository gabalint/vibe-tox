import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { container, sessionWith } from "../testData";
import { ContainerMaterial } from "./ContainerMaterial";
import { observedInstant } from "./ObservedTimeInput";
import { StorageExcursionsPage } from "./StorageExcursionsPage";

const history = {
  moves: [],
  holds: [],
  depletion: null,
  disposal: null,
  hasMore: false,
  snapshot: { maxMoveRowId: "0", maxHoldRowId: "0" },
};
describe("Material workflows", () => {
  beforeEach(mockGridLayout);
  it.each([
    ["assessed", "holds_placed", "Assessed", "Holds placed"],
    ["assessed", "no_impact", "Assessed", "No impact"],
    ["future_state", "future_outcome", "future_state", "future_outcome"],
  ])(
    "retains readable incident labels and unfamiliar values: %s",
    async (state, outcome, stateLabel, outcomeLabel) => {
      const entry = {
        excursionId: "synthetic-incident",
        locationId: "synthetic-location",
        description: "Synthetic temperature observation",
        observedFrom: "2026-10-14T12:00:00.000000000Z",
        observedTo: null,
        recordedBy: "Synthetic recorder",
        recordedAt: "2026-10-14T13:00:00.000000000Z",
        state,
        outcome,
        holds: [],
        affectedContainerLabels: {},
        assessedAt: null,
        assessmentReason: null,
      };
      mockCommands({
        current_session: () => sessionWith(["manage_quality"]),
        list_excursions: () => ({ items: [entry], hasMore: false, snapshot: "1" }),
        list_locations: () => [],
        get_excursion: () => entry,
      });
      renderRoutes(
        [{ path: "/", element: <StorageExcursionsPage /> }],
        "/?excursionId=synthetic-incident",
      );
      expect(
        await screen.findByText(`State: ${stateLabel}. Assessment: ${outcomeLabel}.`),
      ).toBeTruthy();
      expect(await screen.findByRole("gridcell", { name: stateLabel })).toBeTruthy();
    },
  );
  it("explains missing storage setup without granting configuration to receiving staff", async () => {
    mockCommands({
      current_session: () => sessionWith(["accession"]),
      list_excursions: () => ({ items: [], hasMore: false, snapshot: "0" }),
      list_locations: () => [],
    });
    const { user } = renderRoutes([{ path: "/", element: <StorageExcursionsPage /> }], "/");
    await user.click(await screen.findByRole("button", { name: "Record excursion" }));
    expect(
      await screen.findByText(/An administrator must configure the required storage location/),
    ).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Configure storage locations" })).toBeNull();
  });
  it("gives quality an honest empty incident review without configuration or intake actions", async () => {
    mockCommands({
      current_session: () => sessionWith(["manage_quality"]),
      list_excursions: () => ({ items: [], hasMore: false, snapshot: "0" }),
      list_locations: () => [],
    });
    renderRoutes([{ path: "/", element: <StorageExcursionsPage /> }], "/");
    expect(
      await screen.findByText("No recorded incidents are available for quality review."),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record excursion" })).toBeNull();
    expect(screen.queryByRole("link", { name: "Storage locations" })).toBeNull();
  });
  it("lets quality review material without granting accession mutations", async () => {
    mockCommands({
      current_session: () => sessionWith(["manage_quality"]),
      get_container_history: () => history,
    });
    renderRoutes([{ path: "/", element: <ContainerMaterial container={container()} /> }], "/");
    expect(await screen.findByRole("button", { name: "Place hold" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Move container" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Mark depleted" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Dispose container" })).toBeNull();
    expect(screen.getByRole("link", { name: "Storage excursions" })).toBeTruthy();
  });
  it("requires an attributable reason before terminal material action", async () => {
    const deplete = vi.fn(() => ({}));
    mockCommands({
      current_session: () => sessionWith(["accession"]),
      get_container_history: () => history,
      mark_container_depleted: deplete,
    });
    const { user } = renderRoutes(
      [{ path: "/", element: <ContainerMaterial container={container()} /> }],
      "/",
    );
    await user.click(await screen.findByRole("button", { name: "Mark depleted" }));
    await user.click(screen.getByRole("button", { name: "Mark depleted" }));
    expect(deplete).not.toHaveBeenCalled();
    await user.type(
      screen.getByRole("textbox", { name: /Reason/ }),
      "Synthetic exhausted material",
    );
    await user.click(screen.getByRole("button", { name: "Mark depleted" }));
    expect(await screen.findByText("Mark depleted recorded")).toBeTruthy();
    expect(deplete).toHaveBeenCalledWith({
      input: {
        containerId: container().containerId,
        expectedRevision: 1,
        reason: "Synthetic exhausted material",
      },
    });
  });
  it("reaches later incident pages without resolving earlier incidents", async () => {
    const list = vi.fn(() => ({ items: [], hasMore: true, snapshot: "51" }));
    mockCommands({
      current_session: () => sessionWith(["accession"]),
      list_excursions: list,
      list_locations: () => [],
    });
    const { user } = renderRoutes(
      [{ path: "/specimens/storage", element: <StorageExcursionsPage /> }],
      "/specimens/storage",
    );
    await user.click(await screen.findByRole("button", { name: "Next excursions" }));
    expect(list).toHaveBeenCalledWith({ input: { offset: 50, snapshot: "51" } });
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(list).toHaveBeenLastCalledWith({ input: { offset: 0, snapshot: null } });
  });
  it("preserves supplied fractional precision and explicit offset", () => {
    expect(observedInstant("2026-10-13T23:20:00.123456789-04:00")).toBe(
      "2026-10-14T03:20:00.123456789Z",
    );
    expect(observedInstant("2026-10-13T23:20:00")).toBeNull();
    expect(observedInstant("2026-02-31T12:00:00Z")).toBeNull();
  });
});
