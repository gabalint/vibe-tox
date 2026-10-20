import { useMemo, useState } from "react";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { DetailLayout, DetailPanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { MoreActions } from "@/components/RowActions";
import { StatusBadge } from "@/components/StatusBadge";
import { useOpenRecord } from "@/lib/openRecord";
import { openRecordPanel } from "@/lib/openRecordPanel";
import { GallerySection } from "./GallerySection";
import { SYNTHETIC_COLUMNS } from "./GridDemo";
import styles from "./gallery.module.css";
import {
  type SyntheticRow,
  syntheticActions,
  syntheticRowActions,
  syntheticRows,
} from "./syntheticRows";

const ROW_COUNT = 200;

const getRowId = (row: SyntheticRow) => row.id;

/**
 * A directory with a side panel and row actions on synthetic rows: a click or Enter opens a
 * row in the panel (`?open=`), Previous and Next follow the sorted order (also after a sort
 * while the panel is open), Escape closes it, an unknown `?open=` shows "Record not found",
 * and right-click or Shift+F10 opens a row's actions.
 */
export function RecordViewsSection() {
  const rows = useMemo(() => syntheticRows(ROW_COUNT), []);
  const record = useOpenRecord();
  const [lastAction, setLastAction] = useState("none");
  const rowActions = useMemo(() => syntheticRowActions(setLastAction), []);
  const panel = openRecordPanel(record, rows, getRowId, (row) => (
    <SyntheticPanel
      row={row}
      onAction={setLastAction}
      onClose={record.close}
      onPrevious={record.previous}
      onNext={record.next}
    />
  ));
  return (
    <GallerySection title="Side panel and row actions">
      <p role="status">Last action: {lastAction}.</p>
      <div className={styles.fillBox}>
        <DetailLayout panel={panel}>
          <DataGrid
            label="Synthetic directory"
            columns={SYNTHETIC_COLUMNS}
            rows={rows}
            getRowId={getRowId}
            onRowAction={(row, order) => record.open(row.id, order)}
            openRowId={record.openId}
            onOrderChange={record.followOrder}
            rowActions={rowActions}
            fill
          />
        </DetailLayout>
      </div>
    </GallerySection>
  );
}

interface SyntheticPanelProps {
  row: SyntheticRow;
  onAction: (text: string) => void;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/** One synthetic record in view mode, with a primary action and the rest under More. */
function SyntheticPanel({ row, onAction, ...navigation }: SyntheticPanelProps) {
  return (
    <DetailPanel
      identifier={row.id}
      status={<StatusBadge tone={row.status.tone}>{row.status.label}</StatusBadge>}
      actions={
        <>
          <Button variant="primary" onPress={() => onAction(`Edited ${row.id}`)}>
            Edit
          </Button>
          <MoreActions items={syntheticActions(row, onAction)} />
        </>
      }
      {...navigation}
    >
      <Facts
        items={[
          { label: "Panel", value: row.panel },
          { label: "Received", value: row.received },
          { label: "Results", value: row.results },
          { label: "Note", value: row.note },
        ]}
      />
    </DetailPanel>
  );
}
