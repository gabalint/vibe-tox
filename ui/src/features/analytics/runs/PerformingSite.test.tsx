import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderRoutes } from "@/lib/test/router";
import { RUN, sessionWith } from "../testData";
import { PerformingSite } from "./PerformingSite";

function setup(capabilities: string[]) {
  const save = vi.fn(() => ({ ...RUN, laboratoryProfileId: "profile-old" }));
  mockCommands({
    current_session: () => sessionWith(capabilities),
    list_laboratory_profiles: () => [
      {
        profileId: "profile-old",
        name: "Synthetic prior site",
        addressLines: ["100 Synthetic Way"],
        version: 1,
      },
    ],
    record_run_performing_site: save,
  });
  return { ...renderRoutes([{ path: "/", element: <PerformingSite run={RUN} /> }], "/"), save };
}

describe("Performing-site recovery", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("keeps factual site attestation separate from analytical authority", async () => {
    setup(["perform_analysis"]);
    expect(await screen.findByText(/Unknown: resolve/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record performing site" })).toBeNull();
  });
  it("records a selected historical site, evidence and reason with the shown revision", async () => {
    const { user, save } = setup(["perform_analysis", "manage_quality"]);
    await user.click(await screen.findByRole("button", { name: "Record performing site" }));
    const dialog = await screen.findByRole("dialog", { name: "Record performing site" });
    await chooseWithKeyboard(
      user,
      /Historical performing site$/,
      "Synthetic prior site · 100 Synthetic Way · profile 1",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Source evidence reference/ }),
      "Synthetic bench record 42",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Reason/ }),
      "Actual site established from retained run evidence",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record performing site" }));
    expect(await screen.findByText("Performing site recorded")).toBeTruthy();
    expect(save).toHaveBeenCalledWith({
      input: {
        runId: RUN.runId,
        expectedRevision: RUN.revision,
        laboratoryProfileId: "profile-old",
        sourceReference: "Synthetic bench record 42",
        reason: "Actual site established from retained run evidence",
      },
    });
  });
});
