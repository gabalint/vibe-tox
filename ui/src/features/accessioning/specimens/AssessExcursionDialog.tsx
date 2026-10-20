import { useState } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/Button";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import { ReasonField } from "@/components/ReasonField";
import type { AssessExcursionInput } from "@/generated/AssessExcursionInput";
import type { ContainerLookup } from "@/generated/ContainerLookup";
import type { ExcursionView } from "@/generated/ExcursionView";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked, choiceOf } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import { ACCESSION_QUERIES } from "./contract";

const outcomes = [
  { id: "no_impact", label: "No impact" },
  { id: "holds_placed", label: "Place holds on identified containers" },
] as const;
interface Values {
  outcome: string | null;
  containerIds: string[];
  reason: string;
  barcode: string;
}
export function AssessExcursionDialog({
  excursion,
  onClose,
}: {
  excursion: ExcursionView;
  onClose: () => void;
}) {
  return (
    <CommandDialog<Values, AssessExcursionInput>
      title="Assess storage excursion"
      submitLabel="Record assessment"
      command={{ name: "assess_excursion", invalidates: ACCESSION_QUERIES }}
      defaultValues={{ outcome: null, containerIds: [], reason: "", barcode: "" }}
      toInput={(v) => ({
        excursionId: excursion.excursionId,
        expectedRevision: excursion.revision,
        outcome: checked(choiceOf(outcomes, v.outcome)),
        containerIds: v.outcome === "holds_placed" ? v.containerIds : [],
        reason: v.reason.trim(),
      })}
      successTitle="Excursion assessment recorded"
      onClose={onClose}
    >
      <p>
        {excursion.description}. Review the storage records and affected material; the LIS does not
        infer affected containers from a temperature threshold or current location.
      </p>
      <ChoiceInput name="outcome" label="Assessment outcome" options={outcomes} isRequired />
      <AffectedContainers />
      <ReasonField />
    </CommandDialog>
  );
}
function AffectedContainers() {
  const { control, getValues, setValue } = useFormContext<Values>();
  const outcome = useWatch({ control, name: "outcome" });
  const [barcode, setBarcode] = useState("");
  const [selected, setSelected] = useState<{ id: string; label: string }[]>([]);
  const query = useCommandQuery<ContainerLookup>(
    "find_container",
    { input: { labelCode: barcode } },
    { enabled: barcode !== "" },
  );
  if (outcome !== "holds_placed") return null;
  return (
    <>
      <TextInput
        name="barcode"
        label="Affected container barcode"
        description="Scan or type a barcode, then check the returned identity before adding it. Up to 500 containers."
      />
      <Button onPress={() => setBarcode(getValues("barcode").trim())}>
        Find affected container
      </Button>
      {barcode && (
        <QueryContent query={query} what="affected container">
          {(match) => (
            <p>
              {`${match.subject.familyName}, ${match.subject.givenNames ?? ""}`} —{" "}
              {match.accessionNumber} — {match.container.labelCode} ({match.container.state}){" "}
              <Button
                isDisabled={
                  selected.some((r) => r.id === match.container.containerId) ||
                  selected.length >= 500
                }
                onPress={() => {
                  const next = [
                    ...selected,
                    {
                      id: match.container.containerId,
                      label: `${match.accessionNumber} — ${match.container.labelCode}`,
                    },
                  ];
                  setSelected(next);
                  setValue(
                    "containerIds",
                    next.map((r) => r.id),
                  );
                }}
              >
                Add this container
              </Button>
            </p>
          )}
        </QueryContent>
      )}
      <ul>
        {selected.map((r) => (
          <li key={r.id}>
            {r.label}{" "}
            <Button
              onPress={() => {
                const next = selected.filter((v) => v.id !== r.id);
                setSelected(next);
                setValue(
                  "containerIds",
                  next.map((v) => v.id),
                );
              }}
            >
              Remove {r.label}
            </Button>
          </li>
        ))}
      </ul>
      {selected.length === 0 && (
        <p>Select at least one verified container before submitting holds.</p>
      )}
    </>
  );
}
