import { Link } from "react-router";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { QuarantineItemView } from "@/generated/QuarantineItemView";
import { useFormatTime } from "@/lib/format";
import { benchInstrumentPath } from "@/lib/instrumentPages";
import styles from "@/styles/layout.module.css";
import {
  containerLookupPath,
  fileSourcePath,
  RUNS_LIST,
  runPath,
  specimenResultsPath,
  WORKLIST_PATH,
} from "../paths";

/** A place where a cause is fixed: its words and address. */
interface FixLink {
  label: string;
  to: string;
}

/**
 * Where the attempt's source is set up: its file template for a result file read with one,
 * else the instrument's panel under Bench setup, with its sources and their test codes.
 */
function sourceLink(attempt: ImportAttemptDetail): FixLink {
  return attempt.file !== null && attempt.registrationId !== null
    ? { label: "File template", to: fileSourcePath(attempt.instrumentId, attempt.registrationId) }
    : { label: "Instrument in Bench setup", to: benchInstrumentPath(attempt.instrumentId) };
}

/** Container lookup for the row's label, where its accession and specimen are found. */
function lookupLinks(label: string | null): FixLink[] {
  return label === null
    ? []
    : [{ label: `Container lookup for ${label}`, to: containerLookupPath(label) }];
}

/**
 * Where a quarantined row's cause is fixed, by its reason: a label on no run, on the worklist,
 * the open runs or in Container lookup; a label on several runs, those runs; a closed or failed
 * run, the run; a test code or unit, the source; a conflicting or unordered result, the sample's
 * results in review. A value the analyzer sent malformed is fixed at the analyzer: none here.
 */
function rowLinks(
  item: QuarantineItemView,
  attempt: ImportAttemptDetail,
  formatTime: (timestamp: string) => string,
): FixLink[] {
  const { runId, specimenId } = item;
  const results = (label: string) =>
    specimenId === null ? [] : [{ label, to: specimenResultsPath(specimenId) }];
  switch (item.reason) {
    case "unknown_sample":
      return [
        { label: "Worklist", to: WORKLIST_PATH },
        { label: "Open runs", to: RUNS_LIST.href },
        ...lookupLinks(item.sampleLabel),
      ];
    case "ambiguous_sample":
      return [
        ...lookupLinks(item.sampleLabel),
        ...item.matchingRuns.map((match) => ({
          label: `Run created ${formatTime(match.createdAt)}`,
          to: runPath(match.runId),
        })),
      ];
    case "run_closed":
    case "execution_failed":
    case "role_mismatch":
    case "dilution_mismatch":
      return runId === null ? [] : [{ label: "The run", to: runPath(runId) }];
    case "unknown_analyte":
    case "incompatible_unit":
    case "scale_mismatch":
    case "dilution_not_supported":
      return [sourceLink(attempt)];
    case "conflicting_duplicate":
      return results("Existing result in Result review");
    case "not_in_ordered_scope":
      return results("Ordered tests in Result review");
    default:
      return [];
  }
}

/** Where a whole-message or whole-file failure is fixed; none for one fixed at the source. */
function failureLinks(attempt: ImportAttemptDetail): FixLink[] {
  switch (attempt.failureCode) {
    case "unregistered_source":
    case "sender_mismatch":
      return [
        { label: "Instrument in Bench setup", to: benchInstrumentPath(attempt.instrumentId) },
      ];
    case "undecodable_text":
    case "layout_mismatch":
      return [sourceLink(attempt)];
    default:
      return [];
  }
}

function LinkList({ links }: { links: readonly FixLink[] }) {
  return (
    <ul aria-label="Where to fix" className={styles.list}>
      {links.map((link) => (
        <li key={link.to}>
          <Link to={link.to}>{link.label}</Link>
        </li>
      ))}
    </ul>
  );
}

/** The "Where to fix" fact of a quarantined row, or none when nothing here fixes it. */
export function useRowFixFacts(item: QuarantineItemView, attempt: ImportAttemptDetail) {
  const formatTime = useFormatTime();
  return fixFacts(rowLinks(item, attempt, formatTime));
}

/** The "Where to fix" fact of the attempt's failure, or none. */
export function failureFixFacts(attempt: ImportAttemptDetail) {
  return fixFacts(failureLinks(attempt));
}

function fixFacts(links: readonly FixLink[]) {
  return links.length === 0 ? [] : [{ label: "Where to fix", value: <LinkList links={links} /> }];
}
