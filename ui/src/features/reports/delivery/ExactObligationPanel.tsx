import { DetailPanel } from "@/components/DetailPanel";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import type { ActionItem } from "@/components/RowActions";
import { labelOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import type { OpenRecord } from "@/lib/openRecord";
import {
  type DeliveryQueueFilter,
  GET_DELIVERY_OBLIGATION,
  type ObligationView,
} from "../contract";
import { DELIVERY_FILTERS } from "../labels";
import { ObligationPanel } from "./ObligationPanel";

interface Props {
  filter: DeliveryQueueFilter;
  rows: readonly ObligationView[];
  recorded: ObligationView | null;
  record: OpenRecord;
  actions: (obligation: ObligationView) => ActionItem[];
}
/** An explicit URL target can be outside every loaded page or already closed. */
export function ExactObligationPanel({ filter, rows, recorded, record, actions }: Props) {
  const listed = rows.find((row) => row.obligationId === record.openId);
  const kept = !listed && recorded?.obligationId === record.openId ? recorded : undefined;
  const query = useCommandQuery<ObligationView>(
    GET_DELIVERY_OBLIGATION,
    { input: { obligationId: record.openId } },
    { enabled: record.openId !== null && !listed },
  );
  if (record.openId === null) return null;
  const show = (obligation: ObligationView, exact = false) => (
    <ObligationPanel
      refresh={exact ? <RefreshControl query={query} /> : undefined}
      obligation={obligation}
      actions={actions(obligation)}
      leftFilter={kept ? labelOf(DELIVERY_FILTERS, filter) : undefined}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  );
  if (listed) return show(listed);
  if (query.isSuccess) return show(query.data, true);
  if (kept && query.isPending) return show(kept, true);
  return (
    <DetailPanel identifier="Linked delivery obligation" onClose={record.close}>
      <RefreshControl query={query} />
      <QueryContent query={query} what="linked delivery obligation">
        {() => null}
      </QueryContent>
      <p>
        The exact linked record is read independently of queue pages and filters. If it cannot be
        found, return to its report history; no other obligation is selected.
      </p>
    </DetailPanel>
  );
}
