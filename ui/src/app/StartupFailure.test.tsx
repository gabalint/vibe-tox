import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { StartupGate } from "./StartupFailure";

const DATA_FOLDER = "C:\\Users\\synthetic\\AppData\\Local\\org.vibetox.lis";

function renderGate(startupStatus: () => unknown) {
  mockCommands({ startup_status: startupStatus });
  return renderWithProviders(
    <StartupGate>
      <p>The app</p>
    </StartupGate>,
  );
}

describe("StartupGate (D11)", () => {
  it("shows the app while the engine runs", async () => {
    renderGate(() => ({ state: "running", code: null, dataFolder: null }));
    expect(await screen.findByText("The app")).toBeTruthy();
  });

  it("explains a failed start with guidance, the code and the data folder", async () => {
    const { container } = renderGate(() => ({
      state: "failed",
      code: "data_folder_on_network",
      dataFolder: DATA_FOLDER,
    }));
    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "The data folder is on a network drive",
      }),
    ).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("network drives can damage it");
    expect(screen.getByText("data_folder_on_network")).toBeTruthy();
    expect(screen.getByText(DATA_FOLDER)).toBeTruthy();
    expect(screen.queryByText("The app")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("explains a data folder of the other installation (proposed REQ-OPS-016)", async () => {
    renderGate(() => ({
      state: "failed",
      code: "installation_kind_mismatch",
      dataFolder: DATA_FOLDER,
      installation: "laboratory",
    }));
    expect(
      await screen.findByRole("heading", {
        level: 1,
        name: "This data folder belongs to the other installation",
      }),
    ).toBeTruthy();
    expect(screen.getByText("installation_kind_mismatch")).toBeTruthy();
  });

  it("explains a restore that could not be put in place, and that it resumes", async () => {
    const { container } = renderGate(() => ({
      state: "failed",
      code: "restore_apply_failed",
      dataFolder: DATA_FOLDER,
    }));
    expect(
      await screen.findByRole("heading", { name: "The restore could not be put in place" }),
    ).toBeTruthy();
    const steps = screen.getByRole("alert").textContent;
    expect(steps).toContain("Do not delete or move the data folder");
    expect(steps).toContain("the restore continues where it stopped");
    expect(screen.getByText("restore_apply_failed")).toBeTruthy();
    expect(screen.getByText(DATA_FOLDER)).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("explains a data folder whose disk could not be confirmed as local (D15)", async () => {
    renderGate(() => ({ state: "failed", code: "data_folder_unknown", dataFolder: DATA_FOLDER }));
    expect(
      await screen.findByRole("heading", {
        name: "The app could not tell whether the data folder is on this computer",
      }),
    ).toBeTruthy();
    expect(screen.getByRole("alert").textContent).toContain("an internal disk of this computer");
    expect(screen.getByText("data_folder_unknown")).toBeTruthy();
  });

  it("gives general guidance for an internal failure, naming its code", async () => {
    renderGate(() => ({ state: "failed", code: "internal", dataFolder: null }));
    expect(await screen.findByRole("heading", { name: "The app could not start" })).toBeTruthy();
    expect(screen.getByText("internal")).toBeTruthy();
    expect(screen.getByText("Not known")).toBeTruthy();
  });

  it("shows the app when the startup status cannot be read", async () => {
    renderGate(() => Promise.reject(engineError("internal", "No status.")));
    expect(await screen.findByText("The app")).toBeTruthy();
  });
});
