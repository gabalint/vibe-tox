import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import type { ResultLineView } from "@/generated/ResultLineView";
import { classificationText, methodText, resultValueText, reviewStatus } from "../resultLabels";
import review from "./review.module.css";

export const analyteText = (line: ResultLineView) => `${line.analyteName} (${line.analyteCode})`;

const NO_RESULT = "No current result";

/**
 * One row per result line (REQ-RES-001): the current revision's value exactly as received
 * with its units, the cutoff classification, method, revision, review status and how many
 * gates block review. The analyte identifies the line; a click or Enter opens its detail.
 * Result values and analyte names wrap rather than end in "…": a result is never cut off, and
 * an analyte's code stays on one line.
 */
export const RESULT_COLUMNS: readonly DataGridColumn<ResultLineView>[] = [
  {
    id: "analyte",
    header: "Analyte",
    identifier: true,
    cell: (line) => (
      <>
        {line.analyteName} <span className={review.keepTogether}>({line.analyteCode})</span>
        {line.purpose === "validity" && (
          <>
            {" "}
            <StatusBadge tone="info">Specimen validity</StatusBadge>
          </>
        )}
      </>
    ),
    sortValue: analyteText,
    wrap: true,
  },
  {
    id: "result",
    header: "Result",
    cell: (line) => (line.current ? resultValueText(line.current) : NO_RESULT),
    width: 200,
    wrap: true,
  },
  {
    id: "classification",
    header: "Classification",
    cell: (line) => (line.current ? classificationText(line.current) : NO_RESULT),
    sortValue: (line) => (line.current ? classificationText(line.current) : ""),
    wrap: true,
  },
  {
    id: "method",
    header: "Method",
    cell: (line) => (line.current ? methodText(line.current) : NO_RESULT),
  },
  {
    id: "revision",
    header: "Revision",
    cell: (line) => line.current?.revisionNo ?? "None",
    align: "end",
    width: 88,
  },
  {
    id: "review",
    header: "Review",
    cell: (line) => {
      const status = reviewStatus(line.reviewStatus);
      return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
    },
    sortValue: (line) => reviewStatus(line.reviewStatus).label,
    width: 160,
  },
  {
    id: "gates",
    header: "Blocking",
    cell: (line) =>
      line.gates.length === 0 ? (
        0
      ) : (
        <StatusBadge tone="danger">{String(line.gates.length)}</StatusBadge>
      ),
    sortValue: (line) => line.gates.length,
    align: "end",
    width: 96,
  },
];
