import { DataGrid } from "@/components/DataGrid";
import type { ResultLineView } from "@/generated/ResultLineView";
import type { SpecimenResults } from "@/generated/SpecimenResults";
import { useSearchParam } from "@/lib/searchParam";
import styles from "@/styles/layout.module.css";
import split from "../split.module.css";
import { PendingAnalytes } from "./PendingAnalytes";
import { ResultLineDetail } from "./ResultLineDetail";
import { RESULT_COLUMNS } from "./resultColumns";

const getRowId = (line: ResultLineView) => line.resultId;

/**
 * Every result line of the specimen in one grid, so analytes compare side by side, with the
 * open line's evidence, history and actions in a panel beside it, or right under it in a
 * narrow window (REQ-RES-001), then the analytes still waiting for a result. The open line
 * stays in the address; the first line is open until another is chosen. Choosing a line keeps
 * the navigation state, so the queue's breadcrumb and Previous and Next stay.
 */
export function ResultLines({ results }: { results: SpecimenResults }) {
  const [lineId, setLineId] = useSearchParam("line");
  const { lines } = results;
  const openLine = (line: ResultLineView) => setLineId(line.resultId);
  const open = lines.find((line) => line.resultId === lineId) ?? lines[0];
  return (
    <div className={styles.stack}>
      <div className={split.split}>
        <div className={split.list}>
          {lines.length === 0 ? (
            <p className={styles.hint}>No results imported yet.</p>
          ) : (
            <DataGrid
              label="Result lines"
              columns={RESULT_COLUMNS}
              rows={lines}
              getRowId={getRowId}
              onRowAction={openLine}
              openRowId={open?.resultId ?? null}
            />
          )}
        </div>
        {open && (
          <div className={split.detail}>
            <ResultLineDetail key={open.resultId} line={open} />
          </div>
        )}
      </div>
      <PendingAnalytes specimenId={results.specimenId} analytes={results.pendingAnalytes} />
    </div>
  );
}
