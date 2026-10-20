import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { AccessionSpecimenView } from "@/generated/AccessionSpecimenView";
import type { ResolveReceiptChronologyInput } from "@/generated/ResolveReceiptChronologyInput";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import { ACCESSION_QUERIES } from "./contract";
export function ReceiptChronologyDialog({
  specimen,
  onClose,
}: {
  specimen: AccessionSpecimenView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<{ order: (string | null)[]; reason: string }, ResolveReceiptChronologyInput>
      title="Resolve receipt chronology"
      submitLabel="Record reviewed chronology"
      command={{ name: "resolve_receipt_chronology", invalidates: ACCESSION_QUERIES }}
      defaultValues={{ order: specimen.decisions.map(() => null), reason: "" }}
      toInput={(v) => ({
        specimenId: specimen.specimenId,
        expectedRevision: specimen.revision,
        decisionIds: v.order.map(checked),
        reason: v.reason.trim(),
      })}
      successTitle="Reviewed receipt chronology recorded"
      onClose={onClose}
    >
      <p>
        Review trustworthy source evidence to establish every decision from oldest to newest.
        Timestamps alone do not resolve clock rollback or equal times. No order is assumed. If
        evidence is insufficient, cancel and retain the hold.
      </p>
      {specimen.decisions.map((decision, i) => (
        <ChoiceInput
          key={decision.decisionId}
          name={`order.${i}`}
          label={`Decision ${i + 1}${i === 0 ? " (oldest)" : i === specimen.decisions.length - 1 ? " (newest)" : ""}`}
          options={specimen.decisions.map((d) => ({
            id: d.decisionId,
            label: `${d.decision} — ${d.decidedAt} — ${d.decidedBy} — ${d.decisionId}`,
          }))}
          isRequired
        />
      ))}
      <ReasonField />
      <p>
        Include the source reference and how it establishes order. Original decisions and timestamps
        remain unchanged. Duplicate, contradictory and stale submissions are refused.
      </p>
    </CommandDialog>
  );
}
