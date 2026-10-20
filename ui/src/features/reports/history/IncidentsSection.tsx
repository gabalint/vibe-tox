import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import styles from "@/styles/layout.module.css";
import {
  CLOSE_PRESERVATION_INCIDENT,
  type PreservationIncidentView,
  type RevisionSummary,
} from "../contract";
import { INCIDENT_KIND, INCIDENT_RESOLUTIONS, STORED_RECORD } from "../labels";
import { CloseIncidentDialog } from "./CloseIncidentDialog";

interface IncidentsSectionProps {
  incidents: readonly PreservationIncidentView[];
  revisions: readonly RevisionSummary[];
}

/**
 * Issued files and stored report records that failed their digest (NFR-07), open and closed.
 * Damaged content is never shown; quality staff close an incident when the engine allows it.
 */
export function IncidentsSection({ incidents, revisions }: IncidentsSectionProps) {
  const [params] = useSearchParams();
  const selectedId = params.get("incidentId");
  const requestedRevision = params.get("preservationRevisionId");
  const revisionMatches = requestedRevision
    ? incidents.filter((incident) => incident.revisionId === requestedRevision && !incident.closure)
    : [];
  const selected = selectedId
    ? incidents.find((incident) => incident.incidentId === selectedId)
    : revisionMatches.length === 1
      ? revisionMatches[0]
      : undefined;
  const targetRequested = selectedId !== null || requestedRevision !== null;
  const focus = useRef<HTMLLIElement>(null);
  useEffect(() => {
    if (targetRequested && selected) {
      focus.current?.focus();
      focus.current?.scrollIntoView?.({ block: "center" });
    }
  }, [targetRequested, selected]);
  const [closing, setClosing] = useState<PreservationIncidentView | null>(null);
  if (incidents.length === 0 && !targetRequested) return null;
  const subject = (incident: PreservationIncidentView) => {
    if (incident.withdrawalNotice) return "Withdrawal notice";
    const revision = revisions.find((each) => each.revisionId === incident.revisionId);
    return revision ? `Revision ${revision.revisionNo}` : "Unreleased candidate";
  };
  return (
    <Section title="Preservation incidents">
      {targetRequested && !selected && (
        <Alert
          tone="warning"
          title={
            revisionMatches.length > 1 && !selectedId
              ? "Multiple open incidents affect the requested revision"
              : "Requested preservation incident is not on this report"
          }
        >
          Review the incident details and revision, or refresh and return to the queue to check its
          current target. No incident was selected automatically.
        </Alert>
      )}
      <ul aria-label="Preservation incidents" className={styles.list}>
        {incidents.map((incident) => (
          <li
            key={incident.incidentId}
            ref={incident.incidentId === selected?.incidentId ? focus : undefined}
            tabIndex={incident.incidentId === selected?.incidentId ? -1 : undefined}
            aria-label={
              incident.incidentId === selected?.incidentId
                ? "Selected preservation incident"
                : undefined
            }
          >
            {incident.incidentId === selected?.incidentId && <strong>Selected incident. </strong>}
            {subject(incident)}: {STORED_RECORD[incident.record]}{" "}
            {INCIDENT_KIND[incident.kind].toLowerCase()}, detected{" "}
            <Timestamp value={incident.detectedAt} />.{" "}
            {incident.closure ? (
              <>
                Closed <Timestamp value={incident.closure.closedAt} /> by{" "}
                {incident.closure.closedBy.displayName}:{" "}
                {
                  INCIDENT_RESOLUTIONS.find((each) => each.id === incident.closure?.resolution)
                    ?.label
                }
                . Reason: {incident.closure.reason}
              </>
            ) : (
              "Open."
            )}
            {incident.allowedActions.includes(CLOSE_PRESERVATION_INCIDENT) && (
              <Button onPress={() => setClosing(incident)}>Close incident</Button>
            )}
          </li>
        ))}
      </ul>
      {closing && <CloseIncidentDialog incident={closing} onClose={() => setClosing(null)} />}
    </Section>
  );
}
