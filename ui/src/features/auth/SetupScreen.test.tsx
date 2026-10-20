import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthLayout } from "./AuthLayout";
import { SetupScreen } from "./SetupScreen";

const ACCOUNT = {
  username: "demo-tester",
  displayName: "Synthetic Tester",
  password: "synthetic-demo-password",
};

function renderSetup(installation = "demo", create = vi.fn(() => Promise.resolve(null))) {
  const onCreated = vi.fn();
  mockCommands({
    startup_status: () => ({ state: "running", code: null, dataFolder: null, installation }),
    get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
    create_first_administrator: create,
  });
  return {
    ...renderWithProviders(
      <AuthLayout>
        <SetupScreen onCreated={onCreated} />
      </AuthLayout>,
    ),
    create,
    onCreated,
  };
}

async function fillAccount(user: ReturnType<typeof renderSetup>["user"]) {
  await user.type(screen.getByRole("textbox", { name: /Display name/ }), ACCOUNT.displayName);
  await user.type(screen.getByRole("textbox", { name: /Username/ }), ACCOUNT.username);
  await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), ACCOUNT.password);
  await user.type(screen.getByLabelText(/^Confirm password/), ACCOUNT.password);
}

describe("First-run demo setup (REQ-OPS-016)", () => {
  it("waits for the installation kind and offers a retry when it cannot be read", async () => {
    const startup = vi
      .fn()
      .mockRejectedValueOnce(engineError("internal", "Try reading the laboratory again."))
      .mockResolvedValue({ installation: "demo" });
    mockCommands({
      startup_status: startup,
      get_restore_availability: () => ({ available: false, pending: null, lastFailureCode: null }),
    });
    const { user } = renderWithProviders(<SetupScreen onCreated={vi.fn()} />);
    expect(screen.queryByRole("form")).toBeNull();
    await screen.findByText("The laboratory could not be identified");
    expect(screen.queryByRole("form")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(
      await screen.findByRole("checkbox", { name: "Load Standard Catalog and demo data" }),
    ).toBeTruthy();
  });

  it("offers data loading by default and records a one-person demo choice with the account", async () => {
    const { user, create, onCreated, container } = renderSetup();
    const data = await screen.findByRole("checkbox", {
      name: "Load Standard Catalog and demo data",
    });
    expect(data.getAttribute("checked")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /Who will use/ })).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await fillAccount(user);
    await user.click(screen.getByRole("button", { name: "Create demo account" }));
    await waitFor(() => expect(onCreated).toHaveBeenCalledOnce());
    expect(create).toHaveBeenCalledWith({
      input: { ...ACCOUNT, onePerson: true, loadDemoData: true },
    });
  });

  it("lets someone opt out of the dataset without granting every role", async () => {
    const { user, create } = renderSetup();
    await user.click(
      await screen.findByRole("checkbox", { name: "Load Standard Catalog and demo data" }),
    );
    expect(screen.getByText("Who will use this laboratory?")).toBeTruthy();
    await fillAccount(user);
    await user.click(screen.getByRole("button", { name: "Create demo account" }));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({
        input: { ...ACCOUNT, onePerson: false, loadDemoData: false },
      }),
    );
  });

  it("never offers or requests demo data in an ordinary laboratory", async () => {
    const { user, create } = renderSetup("laboratory");
    await screen.findByRole("heading", { name: "Set up this laboratory" });
    expect(screen.queryByRole("checkbox")).toBeNull();
    await fillAccount(user);
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({ input: { ...ACCOUNT, onePerson: false } }),
    );
  });

  it("keeps credentials and the data choice on refusal, with the engine support reference", async () => {
    const create = vi.fn(() =>
      Promise.reject(
        engineError("validation", "Choose another username.", {
          fieldErrors: [
            { field: "username", code: "invalid", message: "Choose another username." },
          ],
        }),
      ),
    );
    const { user, onCreated } = renderSetup("demo", create);
    await screen.findByRole("checkbox");
    await fillAccount(user);
    await user.click(screen.getByRole("button", { name: "Create demo account" }));
    await screen.findByText(/corr-test-0001/);
    expect(screen.getByRole("textbox", { name: /Username/ }).getAttribute("value")).toBe(
      ACCOUNT.username,
    );
    expect(screen.getByRole("checkbox").getAttribute("checked")).not.toBeNull();
    expect(onCreated).not.toHaveBeenCalled();
  });

  it("supports password managers and leaves username spelling untouched", async () => {
    renderSetup();
    await screen.findByRole("checkbox");
    const username = screen.getByRole("textbox", { name: /Username/ });
    expect(username.getAttribute("autocomplete")).toBe("username");
    expect(username.getAttribute("autocapitalize")).toBe("none");
    expect(username.getAttribute("autocorrect")).toBe("off");
    expect(username.getAttribute("spellcheck")).toBe("false");
    expect(
      screen.getByLabelText(/^Password/, { selector: "input" }).getAttribute("autocomplete"),
    ).toBe("new-password");
  });
});
