import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ValidityPolicyView } from "@/generated/ValidityPolicyView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { APPROVED_VERSION, MANAGER, sessionWith, URINE, VERSION_REF } from "../testConfiguration";
import { ValidityPolicyPage } from "./ValidityPolicyPage";

const POLICY: ValidityPolicyView = {
  ...APPROVED_VERSION,
  specimenTypeId: URINE.id,
  specimenTypeCode: URINE.code,
  categories: [
    {
      code: "SYN-DIL",
      label: "Synthetic dilute",
      action: "qualifier",
      qualifierText: "Dilute specimen",
    },
    { code: "SYN-OK", label: "Synthetic acceptable", action: "none", qualifierText: null },
  ],
};

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_specimen_types: () => [{ ...URINE, versionInEffect: 1 }],
    list_validity_policy_versions: () => [APPROVED_VERSION],
    get_validity_policy: () => POLICY,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/specimen-types/:specimenTypeId", element: <ValidityPolicyPage /> }],
    `/configuration/specimen-types/${URINE.id}`,
  );
}

describe("ValidityPolicyPage (REQ-SPC-008)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows a version's categories with their actions and qualifier text", async () => {
    const { container } = renderPage();
    expect(await screen.findByRole("heading", { level: 1, name: "urine Urine" })).toBeTruthy();
    const panel = await screen.findByRole("region", { name: "Version 1" });
    expect(await within(panel).findByText("Report with a qualifier")).toBeTruthy();
    expect(within(panel).getByText("Dilute specimen")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("drafts a policy whose qualifier category carries its text", async () => {
    const draft = vi.fn(() => VERSION_REF);
    const { user } = renderPage({ draft_validity_policy_version: draft });
    await user.click(await screen.findByRole("button", { name: "New draft version" }));
    const dialog = await screen.findByRole("dialog", { name: "New draft version" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Code/ }), "SYN-SUB");
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Label/ }),
      "Synthetic substituted",
    );
    await chooseWithKeyboard(user, /Action$/, "Report with a qualifier");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await within(dialog).findByText("Enter qualifier text.")).toBeTruthy();
    expect(draft).not.toHaveBeenCalled();
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Qualifier text/ }),
      "Substituted specimen",
    );
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    await screen.findByText("Draft version 3 saved");
    expect(draft).toHaveBeenCalledWith({
      input: {
        specimenTypeId: URINE.id,
        content: {
          categories: [
            {
              code: "SYN-SUB",
              label: "Synthetic substituted",
              action: "qualifier",
              qualifierText: "Substituted specimen",
            },
          ],
        },
      },
    });
  });
});
