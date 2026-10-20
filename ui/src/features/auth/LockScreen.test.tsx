import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import type { SessionInfo } from "@/generated/SessionInfo";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { AuthGate } from "./AuthGate";
import { TEST_SESSION } from "./testSession";

const LOCKED: SessionInfo = { ...TEST_SESSION, locked: true };

/** A screen of the app: one query, and one command that may find the session gone. */
function SomeScreen({ session }: { session: SessionInfo }) {
  const data = useCommandQuery<string>("list_worklist");
  const save = useCommandMutation<null>("create_run", { invalidates: [] });
  return (
    <>
      <p>Signed in as {session.displayName}</p>
      <p>{data.data ?? "loading"}</p>
      <Button onPress={() => save.mutate({})}>Save</Button>
    </>
  );
}

function renderGate() {
  return renderWithProviders(<AuthGate>{(session) => <SomeScreen session={session} />}</AuthGate>);
}

describe("Lock screen (REQ-IDN-004)", () => {
  it("unlocks the same person's session with their password", async () => {
    let session = LOCKED;
    const unlock = vi.fn(() => {
      session = TEST_SESSION;
      return TEST_SESSION;
    });
    mockCommands({
      current_session: () => session,
      unlock_session: unlock,
      list_worklist: () => "synthetic worklist",
    });
    const { user, container } = renderGate();
    expect(await screen.findByRole("heading", { level: 1, name: "Session locked" })).toBeTruthy();
    expect(screen.getByText("Test Analyst")).toBeTruthy();
    expect(screen.queryByText("synthetic worklist")).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.type(screen.getByLabelText(/^Password/), "synthetic-password{Enter}");
    expect(await screen.findByText("synthetic worklist")).toBeTruthy();
    expect(unlock).toHaveBeenCalledWith({ input: { password: "synthetic-password" } });
  });

  it("clears the password and stays locked after a wrong password", async () => {
    mockCommands({
      current_session: () => LOCKED,
      unlock_session: () =>
        Promise.reject(engineError("unauthenticated", "The username or password is incorrect.")),
    });
    const { user } = renderGate();
    const password = await screen.findByLabelText<HTMLInputElement>(/^Password/);
    await user.type(password, "wrong-password{Enter}");
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The username or password is incorrect.",
    );
    await waitFor(() => expect(password.value).toBe(""));
    expect(screen.getByRole("heading", { name: "Session locked" })).toBeTruthy();
  });

  it("signs the locked session out so a different person can sign in", async () => {
    let session: SessionInfo | null = LOCKED;
    const signOut = vi.fn(() => {
      session = null;
      return null;
    });
    mockCommands({
      current_session: () => session,
      setup_status: () => ({ needsAdministrator: false }),
      sign_out: signOut,
    });
    const { user } = renderGate();
    await user.click(await screen.findByRole("button", { name: "Sign in as a different user" }));
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: /Username/ })).toHaveProperty("value", "");
    expect(signOut).toHaveBeenCalledTimes(1);
  });
});

describe("Session lost while the app is in use (REQ-IDN-004)", () => {
  it("shows the lock screen when a command reports the session locked", async () => {
    let session = TEST_SESSION;
    mockCommands({
      current_session: () => session,
      list_worklist: () => "synthetic worklist",
      create_run: () => {
        // The engine locked the idle session before this command arrived.
        session = LOCKED;
        return Promise.reject(engineError("session_locked", "The session is locked."));
      },
    });
    const { user } = renderGate();
    expect(await screen.findByText("synthetic worklist")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("heading", { name: "Session locked" })).toBeTruthy();
    expect(screen.queryByText("synthetic worklist")).toBeNull();
  });

  it("shows the sign-in screen when a query reports no session", async () => {
    let session: SessionInfo | null = TEST_SESSION;
    mockCommands({
      current_session: () => session,
      setup_status: () => ({ needsAdministrator: false }),
      list_worklist: () => {
        session = null;
        return Promise.reject(engineError("unauthenticated", "Sign in to continue."));
      },
    });
    renderGate();
    expect(await screen.findByRole("heading", { name: "Sign in" })).toBeTruthy();
    expect(screen.queryByText(/Signed in as/)).toBeNull();
  });
});
