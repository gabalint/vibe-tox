import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { sessionWith } from "../testData";
import { MaterialConfigurationPage } from "./MaterialConfigurationPage";

describe("Material configuration", () => {
  it("completes empty printer setup using an explicit local destination", async () => {
    const save = vi.fn(() => ({}));
    mockCommands({
      current_session: () => sessionWith(["manage_configuration"]),
      list_printers: () => [],
      configure_printer: save,
    });
    const { user } = renderRoutes(
      [{ path: "/setup/material", element: <MaterialConfigurationPage /> }],
      "/setup/material",
    );
    expect(await screen.findByText("No label printer is configured")).toBeTruthy();
    expect(screen.queryByRole("link", { name: "Storage excursions" })).toBeNull();
    await user.click(await screen.findByRole("button", { name: "Add label printer" }));
    await user.type(screen.getByRole("textbox", { name: "Code" }), "bench");
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Synthetic bench printer");
    await user.type(screen.getByRole("textbox", { name: "Printer IP address" }), "192.168.1.20");
    await user.click(screen.getByRole("button", { name: "Save printer" }));
    expect(await screen.findByText("Label printer saved")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        code: "bench",
        name: "Synthetic bench printer",
        transport: "tcp",
        ipAddress: "192.168.1.20",
        port: 9100,
        active: true,
      },
    });
  });
  it("keeps configuration actions unavailable to receiving staff", async () => {
    mockCommands({ current_session: () => sessionWith(["accession"]), list_locations: () => [] });
    renderRoutes(
      [{ path: "/setup/material", element: <MaterialConfigurationPage /> }],
      "/setup/material?tab=locations",
    );
    expect(await screen.findByText("No storage location is configured")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Add storage location" })).toBeNull();
  });
});
