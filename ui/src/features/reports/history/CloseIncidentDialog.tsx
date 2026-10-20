import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import {
  CLOSE_PRESERVATION_INCIDENT,
  type ClosePreservationIncidentInput,
  type IncidentResolution,
  type PreservationIncidentView,
  REPORT_QUERIES,
} from "../contract";
import { INCIDENT_RESOLUTIONS } from "../labels";

interface CloseValues {
  resolution: IncidentResolution | null;
  reason: string;
}

interface CloseIncidentDialogProps {
  incident: PreservationIncidentView;
  onClose: () => void;
}

/**
 * Closes a preservation incident with a resolution and a reason (NFR-07, REQ-OPS-004). The
 * incident and its closure stay on the report; it leaves the incidents queue.
 */
export function CloseIncidentDialog({ incident, onClose }: CloseIncidentDialogProps) {
  return (
    <CommandDialog<CloseValues, ClosePreservationIncidentInput>
      title="Close preservation incident"
      submitLabel="Close incident"
      command={{ name: CLOSE_PRESERVATION_INCIDENT, invalidates: REPORT_QUERIES }}
      defaultValues={{ resolution: null, reason: "" }}
      toInput={({ resolution, reason }) => ({
        incidentId: incident.incidentId,
        expectedRevision: incident.revision,
        resolution: checked(resolution),
        reason: reason.trim(),
      })}
      successTitle="Preservation incident closed"
      onClose={onClose}
    >
      <ChoiceInput name="resolution" label="Resolution" options={INCIDENT_RESOLUTIONS} isRequired />
      <ReasonField />
    </CommandDialog>
  );
}
