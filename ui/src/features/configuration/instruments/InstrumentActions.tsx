import { type ReactNode, useState } from "react";
import type { GridRowActions } from "@/components/DataGrid";
import type { ActionItem } from "@/components/RowActions";
import type { InstrumentChangeResult } from "@/generated/InstrumentChangeResult";
import type { InstrumentConfiguration } from "@/generated/InstrumentConfiguration";
import { pickupName } from "./connectionValues";
import { EnableInstrumentDialog } from "./EnableInstrumentDialog";
import { ChooseFolderDialog, ClearFolderDialog } from "./FolderDialogs";
import { ConnectionDialog } from "./InstrumentDialogs";

type PendingKind = "connection" | "enabled" | "folder" | "clear_folder";
type Pending = { kind: PendingKind; id: string } | null;

/**
 * The actions on each analyzer, for its panel and its row menu, and the one dialog they open.
 * The dialog looks the analyzer up in the latest list, so it always sends the current
 * revision. There is no `allowedActions` on `InstrumentConfiguration` yet: the actions are
 * offered by connection type, as before, and the engine checks `manage_configuration` on each.
 * A result-file instrument adds choosing and clearing its watched folder (F13).
 */
export function useInstrumentActions(
  instruments: readonly InstrumentConfiguration[],
  onSaved: (result: InstrumentChangeResult) => void,
): {
  actionsOf: (instrument: InstrumentConfiguration) => ActionItem[];
  rowActions: GridRowActions<InstrumentConfiguration>;
  dialog: ReactNode;
} {
  const [pending, setPending] = useState<Pending>(null);
  const target = instruments.find((row) => row.id === pending?.id);
  const close = () => setPending(null);
  const actionsOf = (instrument: InstrumentConfiguration): ActionItem[] => {
    const open = (kind: PendingKind) => () => setPending({ kind, id: instrument.id });
    const pickup = pickupName(instrument.connection.kind);
    const items: ActionItem[] = [
      { id: "connection", label: "Edit connection", onAction: open("connection") },
    ];
    if (instrument.connection.kind === "result_file") {
      const hasFolder = instrument.resultFolder !== undefined;
      items.push({
        id: "folder",
        label: hasFolder ? "Change folder" : "Choose folder",
        onAction: open("folder"),
      });
      if (hasFolder) {
        items.push({ id: "clear_folder", label: "Clear folder", onAction: open("clear_folder") });
      }
    }
    items.push({
      id: "enabled",
      label: instrument.enabled ? `Disable ${pickup}` : `Enable ${pickup}`,
      variant: instrument.enabled ? "danger" : undefined,
      onAction: open("enabled"),
    });
    return items;
  };
  const rowActions: GridRowActions<InstrumentConfiguration> = {
    label: (row) => row.code,
    hasItems: () => true,
    items: actionsOf,
  };
  const dialog = target && (
    <>
      {pending?.kind === "connection" && (
        <ConnectionDialog instrument={target} onSaved={onSaved} onClose={close} />
      )}
      {pending?.kind === "enabled" && (
        <EnableInstrumentDialog instrument={target} onSaved={onSaved} onClose={close} />
      )}
      {pending?.kind === "folder" && <ChooseFolderDialog instrument={target} onClose={close} />}
      {pending?.kind === "clear_folder" && (
        <ClearFolderDialog instrument={target} onClose={close} />
      )}
    </>
  );
  return { actionsOf, rowActions, dialog };
}
