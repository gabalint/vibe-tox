import { type InvokeArgs, invoke } from "@tauri-apps/api/core";
import type { CommandName } from "@/generated/CommandName";
import { toCommandError } from "./errors";

/**
 * Calls an engine command. The only place in the UI that imports `invoke` (NFR-31);
 * screens use the hooks in `commands.ts`. Rejects with a `CommandError`.
 */
export async function call<T>(command: CommandName, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toCommandError(error);
  }
}
