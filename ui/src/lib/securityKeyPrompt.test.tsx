import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "@/components/Button";
import { TEST_SESSION } from "@/features/auth/testSession";
import { useCommandMutation } from "./commands";
import { SecurityKeyPrompt } from "./securityKeyPrompt";
import { expectNoA11yViolations } from "./test/a11y";
import { mockCommands } from "./test/ipc";
import { renderWithProviders } from "./test/render";

function BeginOperation() {
  const signIn = useCommandMutation("security_key_sign_in", { invalidates: [] });
  return (
    <>
      <Button onPress={() => signIn.mutate({})}>Verify key</Button>
      <SecurityKeyPrompt />
    </>
  );
}

describe("Native key prompt", () => {
  it("does not poll or collect a PIN while no key operation is pending", () => {
    const status = vi.fn();
    mockCommands({ security_key_status: status });
    renderWithProviders(<BeginOperation />);
    expect(status).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("collects a transient PIN only when requested and binds it to the current operation", async () => {
    const pins: unknown[] = [];
    mockCommands({
      security_key_sign_in: () => new Promise(() => undefined),
      security_key_status: () => ({ operationId: "synthetic-operation-1", status: "pinRequired" }),
      security_key_pin: (args) => {
        pins.push(structuredClone(args));
        return null;
      },
    });
    const { user } = renderWithProviders(<BeginOperation />);
    await user.click(screen.getByRole("button", { name: "Verify key" }));
    const pin = await screen.findByLabelText<HTMLInputElement>(/^Security key PIN/);
    await expectNoA11yViolations(document.body);
    await user.type(pin, "synthetic-pin");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await waitFor(() =>
      expect(pins).toEqual([
        { input: { operationId: "synthetic-operation-1", pin: "synthetic-pin" } },
      ]),
    );
    expect(pin.value).toBe("");
  });

  it("waits for an explicit spare-key handoff without requesting another PIN", async () => {
    const proceed = vi.fn(() => null);
    mockCommands({
      security_key_sign_in: () => new Promise(() => undefined),
      security_key_status: () => ({
        operationId: "synthetic-replacement-phase",
        status: "waitingForReplacement",
      }),
      security_key_continue: proceed,
    });
    const { user } = renderWithProviders(<BeginOperation />);
    await user.click(screen.getByRole("button", { name: "Verify key" }));
    const button = await screen.findByRole("button", { name: "Continue with the new key" });
    expect(screen.queryByLabelText(/^Security key PIN/)).toBeNull();
    expect(proceed).not.toHaveBeenCalled();
    await user.click(button);
    await waitFor(() =>
      expect(proceed).toHaveBeenCalledWith({
        input: { operationId: "synthetic-replacement-phase" },
      }),
    );
  });

  it("cancels the current operation and removes the prompt when native work ends", async () => {
    let finish: (value: unknown) => void = () => undefined;
    const cancel = vi.fn(() => {
      finish(TEST_SESSION);
      return null;
    });
    mockCommands({
      security_key_sign_in: () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
      security_key_status: () => ({
        operationId: "synthetic-operation-2",
        status: "waitingForTouch",
      }),
      security_key_cancel: cancel,
    });
    const { user } = renderWithProviders(<BeginOperation />);
    await user.click(screen.getByRole("button", { name: "Verify key" }));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalledWith({ input: { operationId: "synthetic-operation-2" } });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
});
