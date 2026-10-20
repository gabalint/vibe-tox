import { ChoiceInput } from "@/components/FormFields";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { ReasonField } from "@/components/ReasonField";
import type { ContainerHoldView } from "@/generated/ContainerHoldView";
import type { ContainerView } from "@/generated/ContainerView";
import type { MoveContainerInput } from "@/generated/MoveContainerInput";
import type { PlaceContainerHoldInput } from "@/generated/PlaceContainerHoldInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { ACCESSION_QUERIES } from "./contract";
import { LocationChoice } from "./LocationChoice";
import { useAccessionCommand } from "./useAccessionCommand";
export type MaterialAction = "move" | "hold" | "deplete" | "dispose" | ContainerHoldView;
const kinds = [
  { id: "retention", label: "Retention" },
  { id: "legal", label: "Legal" },
  { id: "quality", label: "Quality" },
] as const;
export function MaterialActionDialog({
  container,
  action,
  onClose,
}: {
  container: ContainerView;
  action: MaterialAction;
  onClose: () => void;
}) {
  if (action === "move")
    return (
      <CommandDialog<{ locationId: string | null; reason: string }, MoveContainerInput>
        title={`Move ${container.labelCode}`}
        submitLabel="Record move"
        command={{ name: "move_container", invalidates: ACCESSION_QUERIES }}
        defaultValues={{ locationId: null, reason: "" }}
        toInput={(v) => ({
          containerId: container.containerId,
          expectedRevision: container.revision,
          locationId: checked(v.locationId),
          reason: v.reason.trim() || undefined,
        })}
        successTitle="Container move recorded"
        onClose={onClose}
      >
        <LocationChoice />
        <ReasonField />
        <p>
          Verify the physical container and destination. A new move establishes its current location
          without rewriting older evidence.
        </p>
      </CommandDialog>
    );
  if (action === "hold")
    return (
      <CommandDialog<{ kind: string | null; reason: string }, PlaceContainerHoldInput>
        title={`Place hold on ${container.labelCode}`}
        submitLabel="Place hold"
        command={{ name: "place_hold", invalidates: ACCESSION_QUERIES }}
        defaultValues={{ kind: null, reason: "" }}
        toInput={(v) => ({
          containerId: container.containerId,
          kind: checked(choiceOf(kinds, v.kind)),
          reason: v.reason.trim(),
        })}
        successTitle="Container hold placed"
        onClose={onClose}
      >
        <ChoiceInput name="kind" label="Hold kind" options={kinds} isRequired />
        <ReasonField />
        <p>
          Open holds block testing, aliquots and disposal. Excursion holds are placed through a
          storage excursion assessment.
        </p>
      </CommandDialog>
    );
  return <TerminalAction container={container} action={action} onClose={onClose} />;
}
function TerminalAction({
  container,
  action,
  onClose,
}: {
  container: ContainerView;
  action: "deplete" | "dispose" | ContainerHoldView;
  onClose: () => void;
}) {
  const release = typeof action === "object";
  const command = useAccessionCommand<{
    containerId?: string;
    holdId?: string;
    expectedRevision: number;
    reason: string;
  }>(
    release
      ? "release_hold"
      : action === "deplete"
        ? "mark_container_depleted"
        : "dispose_container",
  );
  const label = release
    ? "Release hold"
    : action === "deplete"
      ? "Mark depleted"
      : "Dispose container";
  return (
    <ReasonConfirmDialog
      title={`${label}: ${container.labelCode}`}
      confirmLabel={label}
      variant={release ? "primary" : "danger"}
      command={command}
      input={(reason) =>
        release
          ? { holdId: action.holdId, expectedRevision: action.revision, reason }
          : { containerId: container.containerId, expectedRevision: container.revision, reason }
      }
      successTitle={`${label} recorded`}
      onClose={onClose}
    >
      <p>
        {release
          ? `Release the ${action.kind} hold: ${action.reason}. Other open holds remain effective.`
          : action === "deplete"
            ? "Confirm the material is exhausted. This terminal state prevents further testing and aliquots; history remains available."
            : "Confirm physical identity and the authorized disposal. Disposal is permanent in the LIS and requires every hold to have been released."}
      </p>
    </ReasonConfirmDialog>
  );
}
