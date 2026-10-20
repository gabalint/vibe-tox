import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { SessionInfo } from "@/generated/SessionInfo";
import type { SignInStep } from "@/generated/SignInStep";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";
import { TEST_SESSION } from "./testSession";

const NO_SETUP = { needsAdministrator: false };
const NEW_PASSWORD = "synthetic-new-password";

function renderGate() {
  return renderWithProviders(
    <AuthGate aside={<p>Listener status</p>}>
      {(session) => <p>Signed in as {session.displayName}</p>}
    </AuthGate>,
  );
}

type User = ReturnType<typeof renderGate>["user"];

async function submitCredentials(user: User, password = "synthetic-password") {
  await user.type(await screen.findByRole("textbox", { name: /Username/ }), "analyst");
  await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), password);
  await user.click(screen.getByRole("button", { name: "Sign in" }));
}

async function enterCode(user: User, code = "123456") {
  await user.type(await screen.findByRole("textbox", { name: /Verification code/ }), code);
  await user.click(screen.getByRole("button", { name: "Verify" }));
}

async function chooseNewPassword(user: User, confirmation = NEW_PASSWORD) {
  await user.type(await screen.findByLabelText(/^New password/), NEW_PASSWORD);
  await user.type(screen.getByLabelText(/^Confirm new password/), confirmation);
  await user.click(screen.getByRole("button", { name: "Change password" }));
}

/** Engine commands for a sign-in that ends in `TEST_SESSION` once `verify_mfa` succeeds. */
function signInEngine(firstStep: SignInStep, extra: Record<string, () => unknown> = {}) {
  let session: SessionInfo | null = null;
  const verifyMfa = vi.fn(() => {
    session = TEST_SESSION;
    return TEST_SESSION;
  });
  mockCommands({
    current_session: () => session,
    setup_status: () => NO_SETUP,
    sign_in: () => firstStep,
    verify_mfa: verifyMfa,
    ...extra,
  });
  return { verifyMfa };
}

describe("AuthGate first run", () => {
  it("creates the first administrator, then offers sign-in", async () => {
    let needsAdministrator = true;
    const create = vi.fn(() => {
      needsAdministrator = false;
      return null;
    });
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      current_session: () => null,
      setup_status: () => ({ needsAdministrator }),
      create_first_administrator: create,
    });
    const { user, container } = renderGate();
    expect(await screen.findByRole("heading", { name: "Set up this laboratory" })).toBeTruthy();
    expect(screen.getAllByText("Use 12 to 128 characters.").length).toBeGreaterThan(0);
    await expectNoA11yViolations(container, { wholePage: true });

    await user.type(screen.getByRole("textbox", { name: /Username/ }), "admin");
    await user.type(screen.getByRole("textbox", { name: /Display name/ }), "Test Administrator");
    await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), "too-short");
    await user.type(screen.getByLabelText(/^Confirm password/), "too-short");
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    expect(await screen.findByText("Use at least 12 characters.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText(/^Password/, { selector: "input" }));
    await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), NEW_PASSWORD);
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    expect(await screen.findByText("The passwords do not match.")).toBeTruthy();
    expect(create).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText(/^Confirm password/));
    await user.type(screen.getByLabelText(/^Confirm password/), NEW_PASSWORD);
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByText(/The administrator account was created/)).toBeTruthy();
    expect(create).toHaveBeenCalledWith({
      input: {
        username: "admin",
        displayName: "Test Administrator",
        password: NEW_PASSWORD,
        onePerson: false,
      },
    });
  });

  it("sets the laboratory up for one person when only they will use it (ADR 0015)", async () => {
    const create = vi.fn(() => null);
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      current_session: () => null,
      setup_status: () => ({ needsAdministrator: true }),
      create_first_administrator: create,
    });
    const { user } = renderGate();
    await user.type(await screen.findByRole("textbox", { name: /Username/ }), "director");
    await user.type(screen.getByRole("textbox", { name: /Display name/ }), "Test Director");
    await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), NEW_PASSWORD);
    await user.type(screen.getByLabelText(/^Confirm password/), NEW_PASSWORD);
    expect(screen.getByText(/Only me gives this account every role/)).toBeTruthy();
    await chooseWithKeyboard(
      user,
      /Who will use this laboratory\?$/,
      "Only me: I run this laboratory alone",
    );
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({
        input: expect.objectContaining({ username: "director", onePerson: true }),
      }),
    );
  });

  it("shows the engine's refusal when an administrator already exists", async () => {
    mockCommands({
      startup_status: () => ({ installation: "laboratory" }),
      current_session: () => null,
      setup_status: () => ({ needsAdministrator: true }),
      create_first_administrator: () =>
        Promise.reject(engineError("conflict", "An administrator already exists.")),
    });
    const { user } = renderGate();
    await user.type(await screen.findByRole("textbox", { name: /Username/ }), "admin");
    await user.type(screen.getByRole("textbox", { name: /Display name/ }), "Test Administrator");
    await user.type(screen.getByLabelText(/^Password/, { selector: "input" }), NEW_PASSWORD);
    await user.type(screen.getByLabelText(/^Confirm password/), NEW_PASSWORD);
    await user.click(screen.getByRole("button", { name: "Create administrator" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "An administrator already exists.",
    );
  });
});

describe("AuthGate sign-in steps (NFR-19)", () => {
  it("shows the sign-in screen and its aside when no one is signed in", async () => {
    mockCommands({ current_session: () => null, setup_status: () => NO_SETUP });
    const { container } = renderGate();
    expect(await screen.findByRole("heading", { level: 1, name: "Sign in" })).toBeTruthy();
    expect(screen.getByText("Listener status")).toBeTruthy();
    expect(screen.queryByText(/Signed in as/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("signs in with password, then the authenticator code", async () => {
    const signIn = vi.fn(() => ({ kind: "mfaRequired", challengeId: "challenge-1" }));
    const { verifyMfa } = signInEngine(
      { kind: "mfaRequired", challengeId: "challenge-1" },
      {
        sign_in: signIn,
      },
    );
    const { user } = renderGate();
    await submitCredentials(user);
    expect(
      await screen.findByRole("heading", { name: "Enter your verification code" }),
    ).toBeTruthy();
    await enterCode(user);
    expect(await screen.findByText("Signed in as Test Analyst")).toBeTruthy();
    expect(signIn).toHaveBeenCalledWith({
      input: { username: "analyst", password: "synthetic-password" },
    });
    expect(verifyMfa).toHaveBeenCalledWith({
      input: { challengeId: "challenge-1", code: "123456" },
    });
  });

  it("requires a new password before the code when the password is temporary", async () => {
    const change = vi.fn(() => ({ kind: "mfaRequired", challengeId: "challenge-2" }));
    const { verifyMfa } = signInEngine(
      { kind: "passwordChangeRequired", challengeId: "challenge-1" },
      { change_initial_password: change },
    );
    const { user, container } = renderGate();
    await submitCredentials(user);
    expect(await screen.findByRole("heading", { name: "Choose a new password" })).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await chooseNewPassword(user, "something-else-entirely");
    expect(await screen.findByText("The passwords do not match.")).toBeTruthy();
    expect(change).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText(/^Confirm new password/));
    await user.type(screen.getByLabelText(/^Confirm new password/), NEW_PASSWORD);
    await user.click(screen.getByRole("button", { name: "Change password" }));
    await enterCode(user);
    expect(await screen.findByText("Signed in as Test Analyst")).toBeTruthy();
    expect(change).toHaveBeenCalledWith({
      input: { challengeId: "challenge-1", newPassword: NEW_PASSWORD },
    });
    expect(verifyMfa).toHaveBeenCalledWith({
      input: { challengeId: "challenge-2", code: "123456" },
    });
  });

  it("enrolls an authenticator with the engine's QR code and key, then confirms it", async () => {
    const { verifyMfa } = signInEngine({
      kind: "mfaEnrollmentRequired",
      challengeId: "challenge-1",
      enrollment: {
        secretBase32: "JBSWY3DPEHPK3PXP",
        otpauthUri: "otpauth://totp/VibeTox:admin?secret=JBSWY3DPEHPK3PXP",
        qrSvg: '<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>',
      },
    });
    const { user, container } = renderGate();
    await submitCredentials(user);
    const qr = await screen.findByRole("img", { name: "QR code for your authenticator app" });
    expect(qr.getAttribute("src")).toMatch(/^data:image\/svg\+xml;charset=utf-8,%3Csvg/);
    expect(screen.getByText("JBSW Y3DP EHPK 3PXP")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await enterCode(user, "654321");
    expect(await screen.findByText("Signed in as Test Analyst")).toBeTruthy();
    expect(verifyMfa).toHaveBeenCalledWith({
      input: { challengeId: "challenge-1", code: "654321" },
    });
  });

  it("keeps the code step after a wrong code and can go back", async () => {
    signInEngine(
      { kind: "mfaRequired", challengeId: "challenge-1" },
      {
        verify_mfa: () =>
          Promise.reject(
            engineError(
              "unauthenticated",
              "The code is not valid. Enter the current code from your authenticator app.",
            ),
          ),
      },
    );
    const { user } = renderGate();
    await submitCredentials(user);
    await enterCode(user, "000000");
    expect((await screen.findByRole("alert")).textContent).toContain("The code is not valid.");
    expect(screen.queryByText(/Signed in as/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.getByRole("textbox", { name: /Username/ })).toBeTruthy();
  });
});

describe("AuthGate expired sign-in steps (not_found)", () => {
  const EXPIRED = engineError("not_found", "This sign-in step has expired. Sign in again.");

  it("returns to the start of sign-in when the code step expired", async () => {
    const signIn = vi.fn(() => ({ kind: "mfaRequired", challengeId: "challenge-1" }));
    signInEngine(
      { kind: "mfaRequired", challengeId: "challenge-1" },
      { sign_in: signIn, verify_mfa: () => Promise.reject(EXPIRED) },
    );
    const { user, container } = renderGate();
    await submitCredentials(user);
    await enterCode(user);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Sign in again");
    expect(alert.textContent).toContain("expired or was already used");
    expect(screen.getByRole("textbox", { name: /Username/ })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: /Verification code/ })).toBeNull();
    expect(screen.queryByText(/Signed in as/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await submitCredentials(user);
    expect(
      await screen.findByRole("heading", { name: "Enter your verification code" }),
    ).toBeTruthy();
    expect(screen.queryByText(/expired or was already used/)).toBeNull();
    expect(signIn).toHaveBeenCalledTimes(2);
  });

  it("returns to the start of sign-in when the password-change step expired", async () => {
    signInEngine(
      { kind: "passwordChangeRequired", challengeId: "challenge-1" },
      { change_initial_password: () => Promise.reject(EXPIRED) },
    );
    const { user } = renderGate();
    await submitCredentials(user);
    await chooseNewPassword(user);
    expect((await screen.findByRole("alert")).textContent).toContain("Sign in again");
    expect(screen.getByRole("textbox", { name: /Username/ })).toBeTruthy();
    expect(screen.queryByLabelText(/^New password/)).toBeNull();
  });

  it("does not show the expiry notice after going back on purpose", async () => {
    signInEngine({ kind: "mfaRequired", challengeId: "challenge-1" });
    const { user } = renderGate();
    await submitCredentials(user);
    await user.click(await screen.findByRole("button", { name: "Back" }));
    expect(screen.getByRole("textbox", { name: /Username/ })).toBeTruthy();
    expect(screen.queryByText("Sign in again")).toBeNull();
  });
});

describe("AuthGate sign-in failures", () => {
  it("clears the password after a wrong password, keeping the username", async () => {
    mockCommands({
      current_session: () => null,
      setup_status: () => NO_SETUP,
      sign_in: () =>
        Promise.reject(
          engineError("unauthenticated", "The username or password is incorrect.", {
            correlationId: "corr-77",
          }),
        ),
    });
    const { user } = renderGate();
    await submitCredentials(user);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("The username or password is incorrect.");
    expect(alert.textContent).toContain("Support reference: corr-77");
    const password = screen.getByLabelText<HTMLInputElement>(/^Password/);
    await waitFor(() => expect(password.value).toBe(""));
    expect(document.activeElement).toBe(password);
    expect(screen.getByRole("textbox", { name: /Username/ })).toHaveProperty("value", "analyst");
    expect(screen.queryByText(/Signed in as/)).toBeNull();
  });

  it("shows the engine's lockout message", async () => {
    mockCommands({
      current_session: () => null,
      setup_status: () => NO_SETUP,
      sign_in: () =>
        Promise.reject(
          engineError("unauthenticated", "Too many failed attempts. Try again in 15 minutes."),
        ),
    });
    const { user } = renderGate();
    await submitCredentials(user);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Too many failed attempts. Try again in 15 minutes.",
    );
  });

  it("does not call the engine while required fields are empty", async () => {
    const signIn = vi.fn();
    mockCommands({ current_session: () => null, setup_status: () => NO_SETUP, sign_in: signIn });
    const { user } = renderGate();
    await user.click(await screen.findByRole("button", { name: "Sign in" }));
    expect(await screen.findByText("Enter your username.")).toBeTruthy();
    expect(screen.getByText("Enter your password.")).toBeTruthy();
    expect(signIn).not.toHaveBeenCalled();
  });

  it("shows engine field errors on the matching field", async () => {
    mockCommands({
      current_session: () => null,
      setup_status: () => NO_SETUP,
      sign_in: () =>
        Promise.reject(
          engineError("validation", "Check the highlighted fields.", {
            fieldErrors: [
              { field: "username", code: "too_long", message: "Use at most 64 characters." },
            ],
          }),
        ),
    });
    const { user } = renderGate();
    await submitCredentials(user);
    expect(await screen.findByText("Use at most 64 characters.")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /Username/ }).getAttribute("aria-invalid")).toBe(
      "true",
    );
  });

  it("offers sign-in and a retry when the session cannot be checked", async () => {
    const currentSession = vi.fn(() =>
      Promise.reject(engineError("internal", "The app could not complete the action.")),
    );
    mockCommands({ current_session: currentSession, setup_status: () => NO_SETUP });
    const { user } = renderGate();
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Support reference: corr-test-0001",
    );
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(currentSession).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("heading", { level: 1, name: "Sign in" })).toBeTruthy();
    expect(screen.queryByText(/Signed in as/)).toBeNull();
  });

  // Before the engine side is wired, the app still opens on the sign-in screen.
  it("reports missing auth commands without exposing internal text", async () => {
    mockCommands({ current_session: () => null, setup_status: () => NO_SETUP });
    const { user } = renderGate();
    await submitCredentials(user);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(
      "This function is not available in this version of the app.",
    );
    expect(alert.textContent).not.toContain("not found");
  });
});
