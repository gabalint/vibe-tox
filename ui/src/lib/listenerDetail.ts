// Listener and watched-folder detail codes in plain words (REQ-OPS-013; result-files brief §6).
// The one place they are worded: the listener list and the instrument panel both show them.
import type { Tone } from "@/components/tones";
import type { ListenerDetailCode } from "@/generated/ListenerDetailCode";
import type { ListenerState } from "@/generated/ListenerState";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";

/** Each listener state in words, with its badge tone. */
export const LISTENER_STATES: Record<ListenerState, { tone: Tone; label: string }> = {
  disabled: { tone: "neutral", label: "Disabled" },
  listening: { tone: "info", label: "Listening" },
  connected: { tone: "neutral", label: "Connected" },
  receiving: { tone: "info", label: "Receiving" },
  error: { tone: "danger", label: "Error" },
  // An enabled result-file instrument without a watched folder: files are loaded by hand.
  manual_only: { tone: "neutral", label: "Manual loading only" },
};

/** Plain-language reasons; the engine sends only these codes, never payload text. */
const DETAILS: Record<ListenerDetailCode, string> = {
  configuration_disabled: "Turned off in the instrument settings",
  listener_stopped: "Stopped",
  open_failed: "Could not open the connection",
  transport_failed: "Connection failed",
  persistence_failed: "Could not save received data",
  protocol_failed: "Unexpected message from the analyzer",
  au_query_unsupported:
    "The AU analyzer requested tests; automatic requisition lookup is unavailable",
  establishment_timeout: "The analyzer connected but did not start sending",
  unexpected_peer_rejected: "Refused a connection from an unknown address",
  folder_unavailable: "The watched folder cannot be read",
  files_waiting: "Files waiting to be read for more than 5 minutes",
  file_too_large: "A file in the folder is larger than the 20 MiB the LIS reads and was not read",
};

/** Why a listener or watched folder is not working normally, or `null` when it is. */
export function listenerDetailText(
  listener: Pick<ListenerStatusView, "detail" | "filesWaiting" | "filesTooLarge">,
): string | null {
  if (listener.detail === null) return null;
  const waiting = listener.filesWaiting ?? 0;
  if (listener.detail === "files_waiting" && waiting > 0) {
    return `${waiting} ${waiting === 1 ? "file" : "files"} waiting to be read for more than 5 minutes`;
  }
  const tooLarge = listener.filesTooLarge ?? 0;
  if (listener.detail === "file_too_large" && tooLarge > 1) {
    return `${tooLarge} files in the folder are larger than the 20 MiB the LIS reads and were not read`;
  }
  return DETAILS[listener.detail];
}
