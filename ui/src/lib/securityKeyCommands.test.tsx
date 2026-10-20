import { useQueryClient } from "@tanstack/react-query";
import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { TEST_SESSION } from "@/features/auth/testSession";
import { CURRENT_SESSION, useCommandMutation } from "./commands";
import { securityKeySelection } from "./securityKey";
import { engineError, mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";

const INPUT = {
  reportId: "synthetic-report",
  expectedRevision: 4,
  password: "",
  reason: "Synthetic withdrawal",
};
const KEYS = [
  { credentialId: "key-a", label: "Everyday", revision: 1 },
  { credentialId: "key-b", label: "Spare", revision: 1 },
];

function ActionProbe({ selected = "key-b" }: { selected?: string }) {
  const client = useQueryClient();
  const action = useCommandMutation<null, { input: typeof INPUT }>("withdraw_report", {
    invalidates: [],
  });
  return (
    <>
      <Button
        onPress={() => {
          client.setQueryData([CURRENT_SESSION], TEST_SESSION);
          client.setQueryData(securityKeySelection(TEST_SESSION.userId), selected);
          action.mutate({ input: { ...INPUT } });
        }}
      >
        Confirm withdrawal
      </Button>
      {action.error && <p>{action.error.message}</p>}
    </>
  );
}

function PolicyProbe({ confirmation }: { confirmation: string | null }) {
  const action = useCommandMutation<null, { input: { password: string | null } }>(
    "set_security_policy",
    { invalidates: [] },
  );
  return (
    <Button onPress={() => action.mutate({ input: { password: confirmation } })}>
      Save policy
    </Button>
  );
}

describe("Security-key action routing", () => {
  it.each([null, ""])(
    "requests a key confirmation for policy edits only when confirmation is supplied (%s)",
    async (confirmation) => {
      const direct = vi.fn(() => null);
      const keyAction = vi.fn(() => null);
      const status = vi.fn(() => ({ method: "security_key", credentials: KEYS }));
      mockCommands({
        get_authentication_status: status,
        set_security_policy: direct,
        security_key_action: keyAction,
      });
      const { user } = renderWithProviders(<PolicyProbe confirmation={confirmation} />);
      await user.click(screen.getByRole("button", { name: "Save policy" }));
      if (confirmation === null) {
        await waitFor(() => expect(direct).toHaveBeenCalledWith({ input: { password: null } }));
        expect(keyAction).not.toHaveBeenCalled();
        expect(status).not.toHaveBeenCalled();
      } else {
        await waitFor(() =>
          expect(keyAction).toHaveBeenCalledWith({
            input: {
              credentialId: "key-a",
              action: { command: "set_security_policy", input: { password: "" } },
            },
          }),
        );
        expect(direct).not.toHaveBeenCalled();
      }
    },
  );
  it("sends the complete reviewed input with a current, per-user key selection", async () => {
    const keyAction = vi.fn(() => null);
    const passwordAction = vi.fn(() => null);
    const status = vi.fn(() => ({ method: "security_key", credentials: KEYS }));
    mockCommands({
      get_authentication_status: status,
      security_key_action: keyAction,
      withdraw_report: passwordAction,
    });
    const { user } = renderWithProviders(<ActionProbe />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(keyAction).toHaveBeenCalledWith({
        input: {
          credentialId: "key-b",
          action: { command: "withdraw_report", input: INPUT },
        },
      }),
    );
    expect(status).toHaveBeenCalledOnce();
    expect(passwordAction).not.toHaveBeenCalled();
  });

  it("replaces a stale key selection with a currently registered key", async () => {
    const action = vi.fn(() => null);
    mockCommands({
      get_authentication_status: () => ({ method: "security_key", credentials: KEYS }),
      security_key_action: action,
    });
    const { user } = renderWithProviders(<ActionProbe selected="revoked-key" />);
    await user.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(action).toHaveBeenCalledWith({
        input: {
          credentialId: "key-a",
          action: { command: "withdraw_report", input: INPUT },
        },
      }),
    );
  });

  it("retains the existing command for password accounts", async () => {
    const action = vi.fn(() => null);
    const keyAction = vi.fn();
    mockCommands({ withdraw_report: action, security_key_action: keyAction });
    const { user } = renderWithProviders(<ActionProbe />);
    await user.click(screen.getByRole("button"));
    await waitFor(() => expect(action).toHaveBeenCalledWith({ input: INPUT }));
    expect(keyAction).not.toHaveBeenCalled();
  });

  it("does not submit either route when current authentication metadata is unavailable", async () => {
    const action = vi.fn();
    mockCommands({
      get_authentication_status: () =>
        Promise.reject(engineError("internal", "Authentication could not be checked.")),
      withdraw_report: action,
      security_key_action: action,
    });
    const { user } = renderWithProviders(<ActionProbe />);
    await user.click(screen.getByRole("button"));
    await screen.findByText("Authentication could not be checked.");
    expect(action).not.toHaveBeenCalled();
  });
});
