import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { DemoFrame } from "./DemoFrame";

function renderFrame(status: Record<string, unknown>) {
  mockCommands({ startup_status: () => status });
  return renderWithProviders(
    <DemoFrame>
      <p>The screen</p>
    </DemoFrame>,
  );
}

const RUNNING = { state: "running", code: null, dataFolder: null };

describe("DemoFrame (proposed REQ-OPS-016)", () => {
  it("shows the banner above every screen of the demo laboratory", async () => {
    renderFrame({ ...RUNNING, installation: "demo" });
    expect(await screen.findByRole("region", { name: "Demo laboratory" })).toBeTruthy();
    expect(screen.getByText("The screen")).toBeTruthy();
  });

  it("marks the demo laboratory even when its engine did not start", async () => {
    renderFrame({
      state: "failed",
      code: "demo_reset_failed",
      dataFolder: null,
      installation: "demo",
    });
    expect(await screen.findByRole("button", { name: "Leave the demo laboratory" })).toBeTruthy();
  });

  it("leaves a laboratory installation's screens unmarked", async () => {
    renderFrame({ ...RUNNING, installation: "laboratory" });
    expect(await screen.findByText("The screen")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Demo laboratory" })).toBeNull();
    expect(screen.queryByText(/DEMO LABORATORY/)).toBeNull();
  });
});
