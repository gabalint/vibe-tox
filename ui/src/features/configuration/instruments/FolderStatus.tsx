import { StatusBadge } from "@/components/StatusBadge";
import type { Tone } from "@/components/tones";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import type { ListenerStatusView } from "@/generated/ListenerStatusView";
import { useCommandQuery } from "@/lib/commands";
import { listenerDetailText } from "@/lib/listenerDetail";
import styles from "@/styles/layout.module.css";

/**
 * The watched folder of a result-file instrument and whether files are being picked up from
 * it (F13; result-files brief §6): a folder that cannot be read, or files waiting more than
 * five minutes, need someone to act.
 */
export function folderFacts(instrument: InstrumentConfiguration) {
  if (instrument.resultFolder === undefined) {
    return [
      {
        label: "Watched folder",
        value: "None. Files are loaded by hand under Result imports.",
      },
    ];
  }
  return [
    {
      label: "Watched folder",
      value: <span className={styles.code}>{instrument.resultFolder}</span>,
    },
    { label: "Folder pickup", value: <FolderStatus instrumentId={instrument.id} /> },
  ];
}

/** Read from the listener status, which the top bar already keeps fresh. */
function FolderStatus({ instrumentId }: { instrumentId: string }) {
  const statuses = useCommandQuery<ListenerStatusView[]>("list_listener_status");
  if (statuses.isPending) return "Checking";
  if (statuses.isError) return "Status unavailable";
  const status = statuses.data.find((candidate) => candidate.instrumentId === instrumentId);
  if (status === undefined) return "Status unavailable";
  const { tone, label } = pickupState(status);
  return <StatusBadge tone={tone}>{label}</StatusBadge>;
}

function pickupState(status: ListenerStatusView): { tone: Tone; label: string } {
  const detail = listenerDetailText(status);
  if (status.state === "error") return { tone: "danger", label: detail ?? "Error" };
  if (status.detail === "files_waiting" || status.detail === "file_too_large") {
    return { tone: "warning", label: detail ?? "" };
  }
  if (status.state === "disabled") return { tone: "neutral", label: detail ?? "Disabled" };
  return { tone: "neutral", label: "Watching" };
}
