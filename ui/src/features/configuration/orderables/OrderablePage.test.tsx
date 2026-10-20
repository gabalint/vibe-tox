import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { OrderableVersionView } from "@/generated/OrderableVersionView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  ANALYTES,
  APPROVED_VERSION,
  MANAGER,
  METHOD,
  methodVersion,
  sessionWith,
  VERSION_REF,
} from "../testConfiguration";
import { OrderablePage } from "./OrderablePage";

const ORDERABLE = {
  id: "00000000-0000-7000-8000-000000000e01",
  code: "SYN-SCR",
  name: "Synthetic drug screen",
  kind: "panel" as const,
  versionInEffect: 1,
  draftVersion: null,
};

const METHOD_VERSION = methodVersion(APPROVED_VERSION);

const VERSION: OrderableVersionView = {
  ...APPROVED_VERSION,
  id: "00000000-0000-7000-8000-000000000f01",
  orderableId: ORDERABLE.id,
  code: ORDERABLE.code,
  name: ORDERABLE.name,
  kind: "panel",
  components: [
    {
      position: 1,
      purpose: "validity",
      methodVersionId: METHOD_VERSION.id,
      methodCode: METHOD.code,
      methodVersion: 1,
      specimenTypeCode: "urine",
      certainty: "presumptive",
      outputScale: "qualitative",
      analytes: [
        {
          analyteId: ANALYTES[0]?.id ?? "",
          code: "SYN-AMP",
          name: "Synthetic amphetamine",
          loincCode: null,
          drugClassCode: null,
        },
      ],
    },
  ],
};

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(MANAGER),
    list_orderables: () => [ORDERABLE],
    list_orderable_versions: () => [{ ...APPROVED_VERSION, id: VERSION.id }],
    get_orderable_version: () => VERSION,
    list_methods: () => [METHOD],
    list_method_versions: () => [APPROVED_VERSION],
    get_method_version: () => METHOD_VERSION,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/orderables/:orderableId", element: <OrderablePage /> }],
    `/configuration/orderables/${ORDERABLE.id}`,
  );
}

/** The version in effect, which the page shows by default. */
const shownVersion = () => screen.findByRole("region", { name: "Version 1" });

describe("OrderablePage (REQ-ANA-009, REQ-SPC-004)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows each component with its purpose, method version and analytes", async () => {
    const { container } = renderPage();
    expect(
      await screen.findByRole("heading", { level: 1, name: "SYN-SCR Synthetic drug screen" }),
    ).toBeTruthy();
    const panel = await shownVersion();
    expect(await within(panel).findByText("Specimen validity")).toBeTruthy();
    expect(within(panel).getByText("SYN-IA version 1")).toBeTruthy();
    expect(within(panel).getByText("SYN-AMP")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("drafts a version: method, its version, purpose and the analytes reported", async () => {
    const draft = vi.fn(() => VERSION_REF);
    const { user } = renderPage({ draft_orderable_version: draft });
    await user.click(await screen.findByRole("button", { name: "New draft version" }));
    const dialog = await screen.findByRole("dialog", { name: "New draft version" });
    await chooseWithKeyboard(user, /Method$/, "SYN-IA Synthetic immunoassay");
    await chooseWithKeyboard(user, /Method version$/, "Version 1 (approved)");
    await chooseWithKeyboard(user, /Purpose$/, "Analytical (drug result)");
    await user.click(
      await within(dialog).findByRole("checkbox", { name: "SYN-AMP Synthetic amphetamine" }),
    );
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await screen.findByText("Draft version 3 saved")).toBeTruthy();
    expect(draft).toHaveBeenCalledWith({
      input: {
        orderableId: ORDERABLE.id,
        content: {
          components: [
            {
              methodVersionId: METHOD_VERSION.id,
              purpose: "analytical",
              analyteIds: [ANALYTES[0]?.id],
            },
          ],
        },
      },
    });
  });

  it("requires the analytes reported before saving", async () => {
    const draft = vi.fn();
    const { user } = renderPage({ draft_orderable_version: draft });
    await user.click(await screen.findByRole("button", { name: "New draft version" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /Method$/, "SYN-IA Synthetic immunoassay");
    await chooseWithKeyboard(user, /Method version$/, "Version 1 (approved)");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await within(dialog).findByText("Choose at least one analyte.")).toBeTruthy();
    expect(within(dialog).getByText("Choose purpose.")).toBeTruthy();
    expect(draft).not.toHaveBeenCalled();
  });

  it("copies a version's components, with their method, into a new draft", async () => {
    const draft = vi.fn(() => VERSION_REF);
    const { user } = renderPage({ draft_orderable_version: draft });
    const panel = await shownVersion();
    await user.click(
      await within(panel).findByRole("button", { name: "New draft from this version" }),
    );
    const dialog = await screen.findByRole("dialog", { name: "New draft from version 1" });
    expect(
      await within(dialog).findByRole("checkbox", { name: "SYN-AMP Synthetic amphetamine" }),
    ).toHaveProperty("checked", true);
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    await screen.findByText("Draft version 3 saved");
    expect(draft).toHaveBeenCalledWith({
      input: {
        orderableId: ORDERABLE.id,
        content: {
          components: [
            {
              methodVersionId: METHOD_VERSION.id,
              purpose: "validity",
              analyteIds: [ANALYTES[0]?.id],
            },
          ],
        },
      },
    });
  });
});
