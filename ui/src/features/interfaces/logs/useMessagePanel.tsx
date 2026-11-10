import { useNavigate } from "react-router";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { connectionPath, UNROUTED_PATH } from "../paths";
import { MessagePanel } from "./MessagePanel";

/** A log row's ID: its delivery, or its message when it has none (unrouted or received). */
export const messageRowId = (row: PartnerMessageRow) => row.deliveryId ?? row.messageId;

/**
 * How log rows open (ticket #4678): an outgoing message in a side panel (`?open=` takes its
 * delivery ID), an unrouted one in Unrouted messages, a received one on its connection when
 * `opensConnection`. Previous and Next step through the outgoing messages `shown` only.
 * `loaded` resolves the open message, so one kept open outside the filters stays open.
 */
export function useMessagePanel(
  loaded: readonly PartnerMessageRow[] | undefined,
  shown: readonly PartnerMessageRow[],
  opensConnection: boolean,
) {
  const navigate = useNavigate();
  const record = useOpenRecord();
  const outgoing = new Set(shown.flatMap((row) => row.deliveryId ?? []));
  const panelOrder = (order: readonly string[]) => order.filter((id) => outgoing.has(id));
  const open = (row: PartnerMessageRow, order: readonly string[]) => {
    if (row.deliveryId !== null) record.open(row.deliveryId, panelOrder(order));
    else if (row.connection === null) {
      navigate(`${UNROUTED_PATH}?open=${encodeURIComponent(row.messageId)}`);
    } else if (opensConnection) navigate(connectionPath(row.connection.id));
  };
  const panel = openRecordPanel(record, loaded, messageRowId, (row) =>
    row.deliveryId === null ? null : (
      <MessagePanel
        row={row}
        deliveryId={row.deliveryId}
        opensConnection={opensConnection}
        onClose={record.close}
        onPrevious={record.previous}
        onNext={record.next}
      />
    ),
  );
  return {
    panel,
    open,
    openId: record.openId,
    followOrder: (order: readonly string[]) => record.followOrder(panelOrder(order)),
  };
}
