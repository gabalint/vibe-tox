import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { AppInfo } from "@/generated/AppInfo";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AboutPage } from "./AboutPage";

const INFO: AppInfo = {
  appVersion: "0.1.0",
  webviewVersion: "605.1.15",
  platform: "macos-aarch64",
};

describe("AboutPage", () => {
  it("shows the app and web engine versions", async () => {
    mockCommands({ app_info: () => INFO });
    const { container } = renderWithProviders(<AboutPage />);
    expect(screen.getByRole("heading", { level: 1, name: "About this app" })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Loading");
    expect(await screen.findByText("605.1.15")).toBeTruthy();
    expect(screen.getByText("macos-aarch64")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("says when the web engine version is not reported", async () => {
    mockCommands({ app_info: () => ({ ...INFO, webviewVersion: null }) });
    renderWithProviders(<AboutPage />);
    expect(await screen.findByText("Not reported by the operating system")).toBeTruthy();
  });

  it("shows the engine error with its support reference", async () => {
    mockCommands({
      app_info: () =>
        Promise.reject(engineError("internal", "The app could not complete the action.")),
    });
    renderWithProviders(<AboutPage />);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Support reference: corr-test-0001",
    );
  });
});
