import { Fragment } from "react";
import type { ToxReportData } from "@/generated/ToxReportData";
import layout from "@/styles/layout.module.css";
import { OTHER_TEST_STATUS } from "../labels";
import styles from "../reports.module.css";

/**
 * The printed result tables, one per ordered component, and the ordered tests this revision
 * does not report. Values, units, limits and comparison wording are the engine's strings.
 */
export function ResultGroups({ content }: { content: ToxReportData }) {
  return (
    <>
      <h3 className={layout.subheading}>Results</h3>
      {content.resultGroups.length === 0 && <p className={layout.hint}>No results reported.</p>}
      {content.resultGroups.map((group) => (
        <table key={`${group.name}:${group.method}`} className={styles.table}>
          <caption>
            <strong>{group.name}</strong>. {group.method}; {group.certainty}; {group.role}.
            {group.reportNotes && <p>Method information: {group.reportNotes}</p>}
          </caption>
          <thead>
            <tr>
              <th scope="col">Analyte</th>
              <th scope="col">Result</th>
              <th scope="col">Units</th>
              <th scope="col">Limits</th>
              <th scope="col">Comparison</th>
            </tr>
          </thead>
          <tbody>
            {group.rows.map((row) => (
              <Fragment key={row.analyte}>
                <tr>
                  <th scope="row">{row.analyte}</th>
                  <td>{row.result}</td>
                  <td>{row.units}</td>
                  <td>{row.limits}</td>
                  <td>{row.comparison}</td>
                </tr>
                {(row.recordedAt ||
                  (row.performingSites?.length ?? 0) > 0 ||
                  row.referenceInterval) && (
                  <tr>
                    <td colSpan={5}>
                      <p>
                        <strong>{row.analyte} — provenance</strong>
                      </p>
                      <p>
                        Result recorded: {row.recordedAt ?? "Unknown"}. First reported:{" "}
                        {row.firstReportedAt ??
                          "This revision; actual release time in report history"}
                        .
                      </p>
                      {row.referenceInterval && (
                        <p>Reference information: {row.referenceInterval}</p>
                      )}
                      <p>Performing location: {(row.performingSites ?? []).join("; ")}</p>
                    </td>
                  </tr>
                )}
              </Fragment>
            ))}
          </tbody>
        </table>
      ))}
      <h3 className={layout.subheading}>Tests not reported in this revision</h3>
      {content.otherTests.length === 0 ? (
        <p className={layout.hint}>None.</p>
      ) : (
        <ul className={layout.list}>
          {content.otherTests.map((test) => (
            <li key={`${test.name}:${test.status}`}>
              {test.name}: {OTHER_TEST_STATUS[test.status]}
              {test.note && `. ${test.note}`}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
