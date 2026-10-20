// The demo laboratory (proposed REQ-OPS-016). Shell-only commands: the shell records which
// installation the next start opens and restarts the app, so a successful call is followed by
// the window closing and reopening.
import type { CommandName } from "@/generated/CommandName";
import type { InstallationKind } from "@/generated/InstallationKind";
import type { StartupStatus } from "@/generated/StartupStatus";
import { useCommandQuery } from "@/lib/commands";

/** `startup_status() → StartupStatus`; `installation` names the installation this run opened. */
const STARTUP_STATUS = "startup_status" satisfies CommandName;

/** `open_demo_laboratory()`: only from a laboratory installation that is not yet set up. */
export const OPEN_DEMO_LABORATORY = "open_demo_laboratory" satisfies CommandName;
/** `leave_demo_laboratory()`: the laboratory's own installation opens after the restart. */
export const LEAVE_DEMO_LABORATORY = "leave_demo_laboratory" satisfies CommandName;
/** `reset_demo_laboratory()`: the restart deletes the demo data folder, and only it. */
export const RESET_DEMO_LABORATORY = "reset_demo_laboratory" satisfies CommandName;

/** The installation this run opened, once the shell has answered. */
export function useInstallation(): InstallationKind | undefined {
  return useCommandQuery<StartupStatus>(STARTUP_STATUS).data?.installation;
}
