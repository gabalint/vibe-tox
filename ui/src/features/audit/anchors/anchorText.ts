import type { AnchorFinding } from "@/generated/AnchorFinding";
import type { AnchorSummary } from "@/generated/AnchorSummary";
import { isStream, streamLabel } from "../streams";

/** A stream's label, such as "Clinical"; an unknown stream shows as received. */
export function streamText(stream: string): string {
  return isStream(stream) ? streamLabel(stream) : stream;
}

/** What each anchor finding means for the audit trail; unknown codes show as received. */
const MEANINGS: Readonly<Record<string, string>> = {
  anchor_mismatch: "The kept anchor does not match this computer's audit trail",
  anchor_gap: "Anchors are missing between others that were found",
  anchor_conflict: "Two different anchors were found for the same seal",
  unverified_restore_boundary:
    "A restore explains this, but the restore itself is not yet covered by an anchor",
  local_events_missing: "The anchor covers audit events that are no longer on this computer",
  lost_interval: "Work after the restored backup was lost, as expected after a restore",
  unanchored: "Sealed on this computer but not yet anchored",
  foreign_installation: "Anchors of another installation were found and not compared",
  invalid_anchor_line: "Some lines of the anchor files could not be read as anchors",
  unreadable_file: "Some anchor files could not be read",
};

function where({ stream, sealNo, lastSealNo }: AnchorFinding): string | null {
  if (stream === null) return null;
  const streamPart = `${streamText(stream)} stream`;
  if (sealNo === null) return streamPart;
  const last = lastSealNo ?? sealNo;
  return last === sealNo
    ? `${streamPart}, seal ${sealNo}`
    : `${streamPart}, seals ${sealNo}–${last}`;
}

export function anchorFindingText(finding: AnchorFinding): string {
  const meaning = MEANINGS[finding.code] ?? finding.code;
  const place = where(finding);
  return place === null ? `${meaning}.` : `${meaning} (${place}).`;
}

/** The comparison counts for this screen, in the order a reviewer reads them. */
export const COMPARISON_ROWS: readonly [keyof AnchorSummary, string][] = [
  ["compared", "Anchors compared with the audit trail"],
  ["matched", "Matching"],
  ["mismatched", "Not matching"],
  ["localEventsMissing", "Covering events missing here"],
  ["conflicts", "Conflicting anchors for one seal"],
  ["gaps", "Missing between others"],
  ["lostInterval", "Explained by a restore"],
  ["unverifiedRestoreBoundary", "Explained by a restore not yet anchored"],
  ["unanchored", "Seals not yet anchored"],
];
