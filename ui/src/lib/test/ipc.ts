import type { InvokeArgs } from "@tauri-apps/api/core";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { CommandError } from "@/generated/CommandError";

type CommandHandler = (args: InvokeArgs | undefined) => unknown;

/**
 * Answers engine commands from `handlers` at the IPC boundary. Tests use a password
 * account with unlock PIN disabled unless they supply authentication metadata. Other unlisted
 * commands reject the way Tauri does for an unknown command.
 */
export function mockCommands(handlers: Record<string, CommandHandler>) {
  mockIPC((command, args) => {
    if (command === "get_authentication_status" && !handlers[command]) {
      return {
        method: "password_totp",
        credentials: [],
        recoveryAvailable: false,
        allowSecurityKeyTouchOnly: false,
      };
    }
    if (command === "get_unlock_pin_status" && !handlers[command]) {
      return { enabled: false, attemptsRemaining: 0 };
    }
    const handler = handlers[command];
    if (!handler) return Promise.reject(new Error(`command ${command} not found`));
    return handler(args);
  });
}

/** A synthetic engine error for tests. */
export function engineError(
  code: CommandError["code"],
  message: string,
  extra: Partial<CommandError> = {},
): CommandError {
  return {
    code,
    message,
    fieldErrors: [],
    gateReasons: [],
    correlationId: "corr-test-0001",
    ...extra,
  };
}
