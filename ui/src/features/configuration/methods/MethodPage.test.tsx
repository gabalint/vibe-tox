import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, tabTo, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  ANALYTES,
  APPROVED_VERSION,
  APPROVER,
  APPROVER_ID,
  DRAFT_VERSION,
  MANAGER,
  METHOD,
  methodVersion,
  sessionWith,
  URINE,
  VERSION_REF,
} from "../testConfiguration";
import { MethodPage } from "./MethodPage";

type User = ReturnType<typeof renderRoutes>["user"];

const VERSIONS = [APPROVED_VERSION, DRAFT_VERSION];
/** The engine's directory row: version 1 in effect, version 2 awaiting approval. */
const METHOD_WITH_DRAFT = { ...METHOD, draftVersion: 2 };
/** The engine's rows when no draft awaits approval: only version 1, in effect. */
const NO_DRAFT = { list_methods: () => [METHOD], list_method_versions: () => [APPROVED_VERSION] };
const PAGE = `/configuration/methods/${METHOD.id}`;

function renderPage(
  commands: Record<string, (args: unknown) => unknown> = {},
  session = sessionWith(MANAGER),
  path = PAGE,
) {
  mockCommands({
    current_session: () => session,
    list_methods: () => [METHOD_WITH_DRAFT],
    list_method_versions: () => VERSIONS,
    get_method_version: (args) =>
      methodVersion(
        JSON.stringify(args).includes(DRAFT_VERSION.id) ? DRAFT_VERSION : APPROVED_VERSION,
      ),
    list_specimen_types: () => [URINE],
    list_analytes: () => ANALYTES,
    ...commands,
  });
  return renderRoutes(
    [{ path: "/configuration/methods/:methodId", element: <MethodPage /> }],
    path,
  );
}

const versionLink = (version: number) =>
  screen.findByRole("link", { name: new RegExp(`^Version ${version}`) });

/** Shows a version's content by following its link in the Versions list. */
async function showVersion(user: User, version: number) {
  await user.click(await versionLink(version));
  return screen.findByRole("region", { name: `Version ${version}` });
}

describe("MethodPage (REQ-ANA-002, REQ-ANA-009)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the engine's version state in the header and the draft by default", async () => {
    renderPage();
    const heading = await screen.findByRole("heading", { level: 1, name: /SYN-IA/ });
    expect(heading.parentElement?.textContent).toContain("v1 in effect, v2 awaiting approval");
    expect(await screen.findByRole("region", { name: "Version 2" })).toBeTruthy();
    expect((await versionLink(2)).getAttribute("aria-current")).toBe("true");
    expect((await versionLink(1)).getAttribute("aria-current")).toBeNull();
    expect((await versionLink(1)).textContent).toContain("approved by Test Approver");
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByRole("link", { name: "Methods" }).getAttribute("href")).toBe(
      "/configuration/methods",
    );
  });

  it("shows the version in effect by default when no draft awaits approval", async () => {
    renderPage(NO_DRAFT);
    expect(await screen.findByRole("region", { name: "Version 1" })).toBeTruthy();
    const heading = screen.getByRole("heading", { level: 1, name: /SYN-IA/ });
    expect(heading.parentElement?.textContent).toContain("v1 in effect");
  });

  it("shows the version named in the URL, and another one chosen from the list", async () => {
    const { user, router } = renderPage({}, sessionWith(MANAGER), `${PAGE}?version=1`);
    expect(await screen.findByRole("region", { name: "Version 1" })).toBeTruthy();
    await showVersion(user, 2);
    expect(router.state.location.search).toBe("?version=2");
    expect((await versionLink(2)).getAttribute("aria-current")).toBe("true");
  });

  it("shows a version's lifecycle and limits exactly as stored, with units", async () => {
    const { user, container } = renderPage();
    expect(
      await screen.findByRole("heading", { level: 1, name: "SYN-IA Synthetic immunoassay" }),
    ).toBeTruthy();
    const panel = await showVersion(user, 1);
    expect(await within(panel).findByText("500.0 ng/mL")).toBeTruthy();
    expect(within(panel).getByText("25.00 ng/mL")).toBeTruthy();
    expect(within(panel).getByText("50 to 5000 ng/mL")).toBeTruthy();
    expect(within(panel).getByText("Positive at or above the cutoff")).toBeTruthy();
    expect(within(panel).getByText(/by you$/)).toBeTruthy();
    expect(within(panel).getByText("Test Approver")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("drafts a new version from an approved one by keyboard, sending decimals as text", async () => {
    const draft = vi.fn(() => VERSION_REF);
    const { user } = renderPage({ ...NO_DRAFT, draft_method_version: draft });
    await showVersion(user, 1);
    await tabTo(user, await screen.findByRole("button", { name: "New draft from this version" }));
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog", { name: "New draft from version 1" });
    const cutoff = within(dialog).getByRole("textbox", { name: /^Cutoff/ });
    expect(cutoff).toHaveProperty("value", "500.0");
    await tabTo(user, cutoff);
    await user.keyboard("{Control>}a{/Control}300.00");
    await chooseWithKeyboard(user, /Certainty$/, "Definitive (confirmation)");
    await user.type(
      within(dialog).getByRole("textbox", { name: "Report limitations and notes" }),
      "Synthetic approved method limitation",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: "Reference interval" }),
      "Synthetic approved interval",
    );
    await tabTo(user, within(dialog).getByRole("button", { name: "Save draft" }));
    await user.keyboard("{Enter}");
    expect(await screen.findByText("Draft version 3 saved")).toBeTruthy();
    expect(draft).toHaveBeenCalledWith({
      input: {
        methodId: METHOD.id,
        content: {
          specimenTypeId: URINE.id,
          reportNotes: "Synthetic approved method limitation",
          certainty: "definitive",
          outputScale: "qualitative",
          thresholdRule: "at_or_above",
          thresholdRounding: "before_rounding",
          analytes: [
            {
              analyteId: ANALYTES[0]?.id,
              unit: "ng/mL",
              cutoff: "300.00",
              limitOfDetection: "25.00",
              limitOfQuantitation: "50",
              reportableLow: "50",
              reportableHigh: "5000",
              displayScale: 1,
              referenceInterval: "Synthetic approved interval",
              rangePolicy: "report_only",
            },
          ],
        },
      },
    });
  });

  it("checks decimal format before saving and shows the engine's field errors", async () => {
    const draft = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Check the highlighted fields.", {
          fieldErrors: [
            {
              field: "analytes[0].limitOfQuantitation",
              code: "below_lower_limit",
              message: "This limit is below its lower counterpart.",
            },
          ],
        }),
      ),
    );
    const { user } = renderPage({ ...NO_DRAFT, draft_method_version: draft });
    await showVersion(user, 1);
    await user.click(await screen.findByRole("button", { name: "New draft from this version" }));
    const dialog = await screen.findByRole("dialog");
    const cutoff = within(dialog).getByRole("textbox", { name: /^Cutoff/ });
    await user.clear(cutoff);
    await user.type(cutoff, "1e3");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(await within(dialog).findByText("Enter a decimal number, such as 0.50.")).toBeTruthy();
    expect(draft).not.toHaveBeenCalled();
    await user.clear(cutoff);
    await user.type(cutoff, "0.50");
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    expect(
      await within(dialog).findByText("This limit is below its lower counterpart."),
    ).toBeTruthy();
    expect(within(dialog).getByText("corr-test-0001")).toBeTruthy();
  });

  it("approves a draft for someone with approve_configuration", async () => {
    const approve = vi.fn(() => VERSION_REF);
    const { user } = renderPage(
      { approve_method_version: approve },
      sessionWith(APPROVER, APPROVER_ID),
    );
    await screen.findByRole("region", { name: "Version 2" });
    expect(screen.getByText(/^Approval needs someone other than the author, unless/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Edit draft" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New draft version" })).toBeNull();
    await user.click(await screen.findByRole("button", { name: "Approve version" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Approve method version 2?" });
    await user.type(
      within(confirm).getByLabelText("External approval reference (optional)"),
      "  LAB-APPROVAL-17  ",
    );
    await user.click(within(confirm).getByRole("button", { name: "Approve version" }));
    expect(await screen.findByText("Version 2 approved")).toBeTruthy();
    expect(approve).toHaveBeenCalledWith({
      input: {
        methodVersionId: DRAFT_VERSION.id,
        expectedRevision: 1,
        selfApproval: false,
        externalApprovalReference: "LAB-APPROVAL-17",
      },
    });
  });

  // ADR 0015: where the laboratory allows it, the engine asks the author to confirm each time.
  it("asks the author to confirm a self-approval, never pre-ticked, then sends it", async () => {
    const message =
      "You wrote this configuration. Your laboratory allows you to approve it yourself: confirm that no second person reviewed it.";
    const ask = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [{ field: "selfApproval", code: "self_approval_unconfirmed", message }],
    });
    const approve = vi
      .fn()
      .mockRejectedValueOnce(ask)
      .mockRejectedValueOnce(ask)
      .mockReturnValue(VERSION_REF);
    const { user } = renderPage(
      { approve_method_version: approve },
      sessionWith([...MANAGER, ...APPROVER]),
    );
    const input = (selfApproval: boolean) => ({
      input: { methodVersionId: DRAFT_VERSION.id, expectedRevision: 1, selfApproval },
    });
    const approveButton = () => screen.findByRole("button", { name: "Approve version" });
    await user.click(await approveButton());
    let confirm = within(await screen.findByRole("alertdialog"));
    await user.click(confirm.getByRole("button", { name: "Approve version" }));
    let checkbox = await confirm.findByRole("checkbox", { name: /no second person reviewed/ });
    expect(confirm.getByText(message)).toBeTruthy();
    expect(confirm.queryByText("Approval refused")).toBeNull();
    expect(document.activeElement).toBe(checkbox);
    expect(approve).toHaveBeenLastCalledWith(input(false));
    // Closing forgets the confirmation: the next attempt asks again, unticked.
    await user.click(checkbox);
    await user.click(confirm.getByRole("button", { name: "Cancel" }));
    await user.click(await approveButton());
    confirm = within(await screen.findByRole("alertdialog"));
    expect(confirm.queryByRole("checkbox")).toBeNull();
    await user.click(confirm.getByRole("button", { name: "Approve version" }));
    expect(approve).toHaveBeenLastCalledWith(input(false));
    checkbox = await confirm.findByRole("checkbox", { name: /no second person reviewed/ });
    expect(checkbox).toHaveProperty("checked", false);
    await user.click(checkbox);
    await user.click(confirm.getByRole("button", { name: "Approve version" }));
    expect(await screen.findByText("Version 2 approved")).toBeTruthy();
    expect(approve).toHaveBeenLastCalledWith(input(true));
  });

  // Separation of duties: the engine refuses; the screen shows its refusal and changes nothing.
  it("shows the engine's refusal when the author approves their own draft", async () => {
    const approve = vi.fn(() =>
      Promise.reject(engineError("gate_blocked", "the author of a draft cannot approve it")),
    );
    const { user } = renderPage(
      { approve_method_version: approve },
      sessionWith([...MANAGER, ...APPROVER]),
    );
    await user.click(await screen.findByRole("button", { name: "Approve version" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Approve version" }));
    const refusal = await within(confirm).findByRole("alert");
    expect(refusal.textContent).toContain("Approval refused");
    expect(refusal.textContent).toContain("the author of a draft cannot approve it");
    expect(refusal.textContent).toContain("corr-test-0001");
    expect(screen.queryByText("Version 2 approved")).toBeNull();
    expect(approve).toHaveBeenCalledOnce();
  });

  it("offers no approval to a manager without approve_configuration", async () => {
    renderPage();
    expect(await screen.findByRole("button", { name: "Edit draft" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve version" })).toBeNull();
    expect(screen.queryByText(/^Approval needs someone other than the author/)).toBeNull();
  });

  it("offers a manager no draft edit on an approved version, and an approver only Retire", async () => {
    const manager = renderPage(NO_DRAFT, sessionWith(MANAGER), `${PAGE}?version=1`);
    await screen.findByRole("region", { name: "Version 1" });
    expect(await screen.findByRole("button", { name: "New draft from this version" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Edit draft" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Retire version" })).toBeNull();
    manager.unmount();
    renderPage({}, sessionWith(APPROVER), `${PAGE}?version=1`);
    expect(await screen.findByRole("button", { name: "Retire version" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Approve version" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New draft from this version" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New draft version" })).toBeNull();
  });

  it("offers the open draft instead of a new one while a draft awaits approval", async () => {
    const { user, router } = renderPage({}, sessionWith(MANAGER), `${PAGE}?version=1`);
    await screen.findByRole("region", { name: "Version 1" });
    expect(screen.queryByRole("button", { name: "New draft version" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New draft from this version" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Open draft version 2" }));
    expect(await screen.findByRole("region", { name: "Version 2" })).toBeTruthy();
    expect(router.state.location.search).toBe("?version=2");
    expect(await screen.findByRole("button", { name: "Edit draft" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Open draft version 2" })).toBeNull();
    expect(screen.queryByRole("button", { name: "New draft from this version" })).toBeNull();
  });

  it("offers a new draft version when no draft awaits approval", async () => {
    renderPage(NO_DRAFT);
    expect(await screen.findByRole("button", { name: "New draft version" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Open draft version/ })).toBeNull();
  });

  it("edits an open draft with its revision", async () => {
    const update = vi.fn(() => ({ ...DRAFT_VERSION, revision: 2 }));
    const { user } = renderPage({ update_method_draft: update });
    await user.click(await screen.findByRole("button", { name: "Edit draft" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit draft version 2" });
    await user.click(within(dialog).getByRole("button", { name: "Save draft" }));
    await screen.findByText("Draft version 2 saved");
    expect(update).toHaveBeenCalledWith({
      input: expect.objectContaining({ methodVersionId: DRAFT_VERSION.id, expectedRevision: 1 }),
    });
  });

  it("retires an approved version with a reason", async () => {
    const retire = vi.fn(() => VERSION_REF);
    const { user } = renderPage({ retire_method_version: retire }, sessionWith(APPROVER));
    await showVersion(user, 1);
    await user.click(await screen.findByRole("button", { name: "Retire version" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Retire method version 1?" });
    await user.type(within(confirm).getByRole("textbox", { name: /Reason/ }), "Kit withdrawn");
    await user.click(within(confirm).getByRole("button", { name: "Retire version" }));
    expect(await screen.findByText("Version 1 retired")).toBeTruthy();
    expect(retire).toHaveBeenCalledWith({
      input: { methodVersionId: APPROVED_VERSION.id, expectedRevision: 1, reason: "Kit withdrawn" },
    });
  });

  it("shows loading, then an empty version list", async () => {
    renderPage({ list_method_versions: () => [] });
    expect(screen.getByText("Loading versions…")).toBeTruthy();
    expect(await screen.findByText("No version has been drafted yet.")).toBeTruthy();
    expect(
      screen.getByRole("heading", { level: 1, name: "SYN-IA Synthetic immunoassay" }),
    ).toBeTruthy();
  });

  it("shows the engine error with its support reference, and retries", async () => {
    const list = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "The app could not complete the action."))
      .mockReturnValue(VERSIONS);
    const { user } = renderPage({ list_method_versions: list });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The versions could not be loaded");
    expect(alert.textContent).toContain("corr-test-0001");
    expect(screen.getByRole("heading", { level: 1 })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("region", { name: "Version 2" })).toBeTruthy();
  });
});
