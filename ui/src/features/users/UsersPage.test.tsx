import { screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UserSummary } from "@/generated/UserSummary";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { ADMIN, ADMIN_SESSION, ANALYST, FORMER, TEST_ROLES } from "./testUsers";
import { UsersPage } from "./UsersPage";

type User = ReturnType<typeof renderRoutes>["user"];

function renderPage(commands: Record<string, () => unknown> = {}, path = "/users") {
  const listUsers = vi.fn(() => [ADMIN, ANALYST, FORMER]);
  mockCommands({
    list_users: listUsers,
    list_roles: () => TEST_ROLES,
    current_session: () => ADMIN_SESSION,
    ...commands,
  });
  return { ...renderRoutes([{ path: "/users", element: <UsersPage /> }], path), listUsers };
}

function rowOf(username: string): HTMLElement {
  const row = screen.getByRole("gridcell", { name: username }).closest<HTMLElement>("[role=row]");
  if (!row) throw new Error(`no row for ${username}`);
  return row;
}

/** Opens the user's panel from its row and returns the panel. */
async function openUser(user: User, account: UserSummary) {
  await openRow(user, account.username);
  return screen.findByRole("region", { name: account.displayName });
}

async function waitForUser(username: string) {
  await screen.findByRole("gridcell", { name: username });
  return username;
}

describe("UsersPage (REQ-IDN-004)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("lists users with their roles, authenticator, status and lockout", async () => {
    const { container } = renderPage();
    await waitForUser("admin");
    const analyst = within(rowOf("analyst"));
    expect(analyst.getByText("Analyst")).toBeTruthy();
    expect(analyst.getByText("Not set up")).toBeTruthy();
    expect(analyst.getByText("Locked out")).toBeTruthy();
    expect(within(rowOf("former")).getByText("Disabled")).toBeTruthy();
    expect(within(rowOf("former")).getByText("No roles")).toBeTruthy();
    // No action waits for a selection, and no hint explains the grid.
    expect(screen.queryByRole("button", { name: "Edit roles" })).toBeNull();
    expect(screen.queryByText(/Select/)).toBeNull();
    expect(screen.getByText("3 users")).toBeTruthy();
    await expectNoA11yViolations(container);
  });

  it("says who can help, without a retry, when the roles do not allow the list", async () => {
    renderPage({
      list_users: () => Promise.reject(engineError("forbidden", "You cannot manage users.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot manage users.");
    expect(alert.textContent).toContain("A laboratory administrator can change roles.");
    expect(alert.textContent).toContain("Support reference: corr-test-0001");
    expect(screen.getByRole("searchbox", { name: "Find users" })).toBeTruthy();
    expect(screen.queryByText("0 users")).toBeNull();
    expect(screen.queryByRole("button", { name: "Try again" })).toBeNull();
  });

  it("retries when the engine could not answer", async () => {
    const listUsers = vi
      .fn()
      .mockRejectedValueOnce(engineError("unavailable", "The database is busy."))
      .mockReturnValue([ADMIN]);
    const { user } = renderPage({ list_users: listUsers });
    await user.click(await screen.findByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("gridcell", { name: "admin" })).toBeTruthy();
  });

  it("shows loading, then an empty state", async () => {
    renderPage({ list_users: () => [] });
    expect(screen.getByRole("heading", { level: 1, name: "Users" })).toBeTruthy();
    expect(screen.getByText("Loading users…")).toBeTruthy();
    expect(screen.getByRole("searchbox", { name: "Find users" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Status/ })).toBeTruthy();
    expect(screen.queryByText("0 users")).toBeNull();
    expect(await screen.findByText("No users yet")).toBeTruthy();
  });

  it("creates a user with roles and a temporary password, then opens it", async () => {
    const created: UserSummary = {
      ...ANALYST,
      userId: "00000000-0000-7000-8000-000000000104",
      username: "reviewer",
      displayName: "Test Reviewer",
      roles: ["auditor"],
    };
    const create = vi.fn(() => created);
    const { user, listUsers, router } = renderPage({ create_user: create });
    // The first list has loaded by now; the list after the change holds the new user.
    listUsers.mockReturnValue([ADMIN, created]);
    await waitForUser("admin");
    await user.click(await screen.findByRole("button", { name: "Create user" }));
    const dialog = await screen.findByRole("dialog", { name: "Create user" });
    expect(within(dialog).getByText("Allows: View the audit trail")).toBeTruthy();
    await expectNoA11yViolations(dialog);

    await user.type(within(dialog).getByLabelText(/^Temporary password/), "short");
    await user.click(within(dialog).getByRole("button", { name: "Create user" }));
    expect(await within(dialog).findByText("Enter a username.")).toBeTruthy();
    expect(within(dialog).getByText("Use at least 12 characters.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();

    await user.type(within(dialog).getByRole("textbox", { name: /Username/ }), "reviewer");
    await user.type(within(dialog).getByRole("textbox", { name: /Display name/ }), "Test Reviewer");
    await user.click(within(dialog).getByRole("checkbox", { name: "Auditor" }));
    await user.type(within(dialog).getByLabelText(/^Temporary password/), "-temporary-1");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic passphrase 1");
    await user.click(within(dialog).getByRole("button", { name: "Create user" }));
    expect(await screen.findByText("Test Reviewer was added")).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: {
        username: "reviewer",
        displayName: "Test Reviewer",
        roles: ["auditor"],
        temporaryPassword: "short-temporary-1",
        password: "synthetic passphrase 1",
      },
    });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(listUsers).toHaveBeenCalledTimes(2);
    expect(await screen.findByRole("region", { name: "Test Reviewer" })).toBeTruthy();
    expect(router.state.location.search).toBe(`?open=${created.userId}`);
  });

  it("changes roles with a reason and the user's revision", async () => {
    const setRoles = vi.fn(() => ({ ...ANALYST, roles: ["analyst", "auditor"], revision: 2 }));
    const { user } = renderPage({ set_user_roles: setRoles });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Edit roles" }));
    const dialog = await screen.findByRole("dialog", { name: "Edit roles of Test Analyst" });
    expect(within(dialog).getByRole("checkbox", { name: "Analyst" })).toHaveProperty(
      "checked",
      true,
    );
    await user.click(within(dialog).getByRole("checkbox", { name: "Auditor" }));
    await user.click(within(dialog).getByRole("button", { name: "Save roles" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(setRoles).not.toHaveBeenCalled();

    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Covers audits");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic passphrase 1");
    await user.click(within(dialog).getByRole("button", { name: "Save roles" }));
    expect(await screen.findByText("Roles changed for Test Analyst")).toBeTruthy();
    expect(setRoles).toHaveBeenCalledWith({
      input: {
        userId: ANALYST.userId,
        roles: ["analyst", "auditor"],
        expectedRevision: 1,
        reason: "Covers audits",
        password: "synthetic passphrase 1",
      },
    });
  });

  it("offers a reload when the user changed since the list was loaded (conflict)", async () => {
    const { user, listUsers } = renderPage({
      set_user_roles: () =>
        Promise.reject(
          engineError("conflict", "Someone else changed this user. Reload and try again."),
        ),
    });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Edit roles" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Covers audits");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic passphrase 1");
    await user.click(within(dialog).getByRole("button", { name: "Save roles" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.textContent).toContain("The change was not saved");
    expect(alert.textContent).toContain("Someone else changed this user. Reload and try again.");
    await user.click(within(dialog).getByRole("button", { name: "Reload users" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(listUsers).toHaveBeenCalledTimes(2);
  });

  it("disables a user after confirmation with a reason", async () => {
    const disable = vi.fn(() => ({ ...ANALYST, status: "disabled", revision: 2 }));
    const { user } = renderPage({ disable_user: disable });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Disable user" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Disable Test Analyst?" });
    expect(dialog.textContent).toContain("will no longer be able to sign in");
    await expectNoA11yViolations(dialog);
    await user.click(within(dialog).getByRole("button", { name: "Disable user" }));
    expect(await within(dialog).findByText("Enter the reason for this change.")).toBeTruthy();
    expect(disable).not.toHaveBeenCalled();

    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Left the lab");
    await user.click(within(dialog).getByRole("button", { name: "Disable user" }));
    expect(await screen.findByText("Test Analyst was disabled")).toBeTruthy();
    expect(disable).toHaveBeenCalledWith({
      input: { userId: ANALYST.userId, expectedRevision: 1, reason: "Left the lab" },
    });
  });

  it("shows the engine's reason when the status is already set (conflict)", async () => {
    const enable = vi.fn(() =>
      Promise.reject(engineError("conflict", "The account already has this status.")),
    );
    const { user, listUsers } = renderPage({ enable_user: enable });
    const panel = await openUser(user, FORMER);
    await user.click(within(panel).getByRole("button", { name: "Enable user" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Enable Test Former Analyst?" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Returned");
    await user.click(within(dialog).getByRole("button", { name: "Enable user" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.textContent).toContain("The change was not saved");
    expect(alert.textContent).toContain("The account already has this status.");
    expect(enable).toHaveBeenCalledWith({
      input: { userId: FORMER.userId, expectedRevision: 5, reason: "Returned" },
    });
    await user.click(within(dialog).getByRole("button", { name: "Reload users" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(listUsers).toHaveBeenCalledTimes(2);
  });

  it("keeps the last active administrator and says why (conflict)", async () => {
    const disable = vi.fn(() =>
      Promise.reject(engineError("conflict", "At least one active administrator must remain.")),
    );
    const { user } = renderPage({ disable_user: disable });
    const panel = await openUser(user, ADMIN);
    await user.click(within(panel).getByRole("button", { name: "Disable user" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Disable Test Administrator?" });
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Leaving");
    await user.click(within(dialog).getByRole("button", { name: "Disable user" }));
    const alert = await within(dialog).findByRole("alert");
    expect(alert.textContent).toContain("At least one active administrator must remain.");
    expect(alert.textContent).not.toContain("Someone else changed");
    expect(disable).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Test Administrator was disabled")).toBeNull();
  });

  it("resets sign-in with a temporary password and, if asked, the authenticator", async () => {
    const reset = vi.fn(() => ({ ...ANALYST, mfaEnrolled: false, revision: 2 }));
    const { user } = renderPage({ reset_user_credentials: reset });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Reset sign-in" }));
    const dialog = await screen.findByRole("dialog", { name: "Reset sign-in of Test Analyst" });
    await user.type(within(dialog).getByLabelText(/^Temporary password/), "synthetic-temp-1");
    await user.click(within(dialog).getByRole("checkbox", { name: /authenticator/ }));
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Lost phone");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic passphrase 1");
    await user.click(within(dialog).getByRole("button", { name: "Reset sign-in" }));
    expect(await screen.findByText("Sign-in reset for Test Analyst")).toBeTruthy();
    expect(reset).toHaveBeenCalledWith({
      input: {
        userId: ANALYST.userId,
        temporaryPassword: "synthetic-temp-1",
        resetMfa: true,
        expectedRevision: 1,
        reason: "Lost phone",
        password: "synthetic passphrase 1",
      },
    });
  });

  it("shows a wrong password on the password field and changes nothing", async () => {
    const setRoles = vi.fn(() =>
      Promise.reject(
        engineError("validation", "The password is incorrect.", {
          fieldErrors: [
            { field: "password", code: "incorrect", message: "The password is incorrect." },
          ],
        }),
      ),
    );
    const { user } = renderPage({ set_user_roles: setRoles });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Edit roles" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Covers audits");
    await user.type(within(dialog).getByLabelText(/^Your password/), "wrong passphrase");
    await user.click(within(dialog).getByRole("button", { name: "Save roles" }));
    expect(await within(dialog).findAllByText("The password is incorrect.")).not.toHaveLength(0);
    expect(screen.queryByText("Roles changed for Test Analyst")).toBeNull();
  });

  it("offers only assignable roles and keeps held roles the person cannot change", async () => {
    const setRoles = vi.fn(() => ({ ...ANALYST, roles: ["auditor", "releaser"], revision: 2 }));
    const releaser: UserSummary = { ...ANALYST, roles: ["analyst", "releaser"] };
    const { user } = renderPage({
      list_users: () => [ADMIN, releaser, FORMER],
      set_user_roles: setRoles,
    });
    const panel = await openUser(user, ANALYST);
    await user.click(within(panel).getByRole("button", { name: "Edit roles" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).queryByRole("checkbox", { name: "Authorized releaser" })).toBeNull();
    expect(
      within(dialog).getByText("Also holds Authorized releaser, which you cannot change."),
    ).toBeTruthy();
    await user.click(within(dialog).getByRole("checkbox", { name: "Analyst" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "Auditor" }));
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Moved to audit");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic passphrase 1");
    await user.click(within(dialog).getByRole("button", { name: "Save roles" }));
    await screen.findByText("Roles changed for Test Analyst");
    expect(setRoles).toHaveBeenCalledWith({
      input: expect.objectContaining({ roles: ["auditor", "releaser"] }),
    });
  });

  it("offers no change of the signed-in person's own roles or sign-in, and says so", async () => {
    const { user } = renderPage();
    const panel = within(await openUser(user, ADMIN));
    expect(
      panel.getByText("Ask someone else to change your own roles or reset your sign-in."),
    ).toBeTruthy();
    expect(panel.queryByRole("button", { name: "Edit roles" })).toBeNull();
    expect(panel.queryByRole("button", { name: "Reset sign-in" })).toBeNull();
    expect(panel.getByRole("button", { name: "Disable user" })).toBeTruthy();
    const menu = within(await openRowActions(user, "admin"));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Disable user"]);
  });

  it("offers a quality manager role changes only", async () => {
    const { user } = renderPage({
      current_session: () => ({
        ...ADMIN_SESSION,
        userId: "00000000-0000-7000-8000-000000000199",
        capabilities: ["manage_quality", "approve_configuration"],
      }),
    });
    const panel = within(await openUser(user, ADMIN));
    expect(await panel.findByRole("button", { name: "Edit roles" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Create user" })).toBeNull();
    expect(panel.queryByRole("button", { name: "Disable user" })).toBeNull();
    expect(panel.queryByRole("button", { name: "Reset sign-in" })).toBeNull();
    expect(panel.queryByText(/Ask someone else/)).toBeNull();
  });

  it("offers the panel's actions in the row menu, enabling a disabled user", async () => {
    const { user } = renderPage();
    await waitForUser("former");
    const menu = within(await openRowActions(user, "former"));
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Edit roles",
      "Enable user",
      "Reset sign-in",
    ]);
    await user.click(menu.getByRole("menuitem", { name: "Enable user" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Enable Test Former Analyst?" }),
    ).toBeTruthy();
  });

  it("opens a user with Enter, and Escape closes the panel with focus back on the row", async () => {
    const { user, router, container } = renderPage();
    await waitForUser("analyst");
    await openRowWithEnter(user, "analyst");
    const panel = await screen.findByRole("region", { name: "Test Analyst" });
    expect(router.state.location.search).toBe(`?open=${ANALYST.userId}`);
    expect(within(panel).getByText("Analyst")).toBeTruthy();
    expect(within(panel).getByText("Locked out")).toBeTruthy();
    expect(rowOf("analyst").getAttribute("aria-current")).toBeTruthy();
    await expectNoA11yViolations(container);
    await user.click(within(panel).getByRole("heading", { name: "Test Analyst" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("region", { name: "Test Analyst" })).toBeNull());
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("gridcell", { name: "analyst" })),
    );
  });

  it("opens the user named in the address", async () => {
    renderPage({}, `/users?open=${FORMER.userId}`);
    const panel = await screen.findByRole("region", { name: "Test Former Analyst" });
    expect(within(panel).getByRole("button", { name: "Enable user" })).toBeTruthy();
  });

  it("finds users by name or user name, filters by status, and clears both", async () => {
    const { user, router } = renderPage();
    await waitForUser("admin");
    await user.type(screen.getByRole("searchbox", { name: "Find users" }), "former{Enter}");
    expect(await screen.findByText("1 of 3 users")).toBeTruthy();
    expect(screen.queryByRole("gridcell", { name: "admin" })).toBeNull();
    await user.clear(screen.getByRole("searchbox", { name: "Find users" }));
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("button", { name: /Status/ }));
    await user.click(await screen.findByRole("option", { name: "Disabled" }));
    expect(router.state.location.search).toBe("?status=disabled");
    expect(await screen.findByText("1 of 3 users")).toBeTruthy();
    await user.type(screen.getByRole("searchbox", { name: "Find users" }), "admin{Enter}");
    expect(await screen.findByText("No users match 'admin'")).toBeTruthy();
    // The find field has its own clear button; the no-results state's comes after it.
    const [, clearSearch] = screen.getAllByRole("button", { name: "Clear search" });
    if (!clearSearch) throw new Error("no Clear search in the no-results state");
    await user.click(clearSearch);
    await user.click(await screen.findByRole("button", { name: "Clear filters" }));
    expect(await screen.findByText("3 users")).toBeTruthy();
    expect(router.state.location.search).toBe("");
  });

  it("offers Set up for one person only to the sole active account, then reads the session again", async () => {
    const first = renderPage();
    await waitForUser("analyst");
    expect(screen.queryByRole("button", { name: "Set up for one person" })).toBeNull();
    first.unmount();

    const session = vi.fn(() => ADMIN_SESSION);
    const designate = vi.fn(() => ({ ...ADMIN, roles: ["administrator", "analyst"] }));
    const list = vi.fn(() => [ADMIN, FORMER]);
    const { user } = renderPage({
      list_users: list,
      current_session: session,
      designate_one_person_laboratory: designate,
    });
    await user.click(await screen.findByRole("button", { name: "Set up for one person" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up for one person" });
    expect(within(dialog).getByText(/Your account gets every role/)).toBeTruthy();
    const reads = { session: session.mock.calls.length, users: list.mock.calls.length };
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "I run it alone");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Set up for one person" }));
    expect(await screen.findByText("Your account now has every role")).toBeTruthy();
    expect(designate).toHaveBeenCalledWith({
      input: { reason: "I run it alone", password: "synthetic-secret" },
    });
    await waitFor(() => expect(session.mock.calls.length).toBeGreaterThan(reads.session));
    await waitFor(() => expect(list.mock.calls.length).toBeGreaterThan(reads.users));
  });

  it("shows the engine's refusal while another account is active", async () => {
    const refusal =
      "Only the laboratory's one active account can set it up for one person. Disable the other accounts first.";
    const { user } = renderPage({
      list_users: () => [ADMIN],
      designate_one_person_laboratory: () => Promise.reject(engineError("conflict", refusal)),
    });
    await user.click(await screen.findByRole("button", { name: "Set up for one person" }));
    const dialog = await screen.findByRole("dialog", { name: "Set up for one person" });
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "I run it alone");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Set up for one person" }));
    expect(await within(dialog).findByText(refusal)).toBeTruthy();
  });
});
