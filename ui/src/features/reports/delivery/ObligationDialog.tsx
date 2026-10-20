import { ChoiceInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import { CommandDialog } from "@/lib/CommandDialog";
import { checked } from "@/lib/choices";
import { useCommandQuery } from "@/lib/commands";
import {
  ADD_DELIVERY_OBLIGATION,
  DELIVERY_QUERIES,
  type DestinationView,
  LIST_DELIVERY_DESTINATIONS,
  type ObligationView,
  REDIRECT_DELIVERY_OBLIGATION,
} from "../contract";

/** What the dialog does, and the order's client whose destinations it offers. */
type Mode =
  | { kind: "add"; revisionId: string; clientId: string }
  | { kind: "redirect"; obligationId: string; clientId: string };

interface ObligationValues {
  destinationId: string | null;
  reason: string;
}

const TEXT = {
  add: {
    title: "Add a destination",
    submit: "Add destination",
    success: "Delivery obligation added",
    intro: "The current revision will also be sent to the destination you choose.",
  },
  redirect: {
    title: "Redirect delivery",
    submit: "Redirect",
    success: "Delivery redirected",
    intro: "This obligation closes and a new one is created for the destination you choose.",
  },
};

/**
 * Adds a delivery obligation for the current revision, or redirects an open one
 * (REQ-RPT-003, REQ-RPT-012). Only the order's client's verified destinations are offered;
 * the engine checks again that the destination is verified, active and the order's client's,
 * and records the reason.
 */
export function ObligationDialog({ mode, onClose }: { mode: Mode; onClose: () => void }) {
  const destinations = useCommandQuery<DestinationView[]>(LIST_DELIVERY_DESTINATIONS, {
    input: { clientId: mode.clientId, state: "verified" },
  });
  const options = (destinations.data ?? []).map((each) => ({
    id: each.destinationId,
    label: each.address ? `${each.label} (${each.address})` : each.label,
  }));
  const text = TEXT[mode.kind];
  const toInput = ({ destinationId, reason }: ObligationValues) => {
    const target = { destinationId: checked(destinationId), reason: reason.trim() };
    return mode.kind === "add"
      ? { revisionId: mode.revisionId, ...target }
      : { obligationId: mode.obligationId, ...target };
  };
  return (
    <CommandDialog<ObligationValues, ReturnType<typeof toInput>, ObligationView>
      title={text.title}
      submitLabel={text.submit}
      command={{
        name: mode.kind === "add" ? ADD_DELIVERY_OBLIGATION : REDIRECT_DELIVERY_OBLIGATION,
        invalidates: DELIVERY_QUERIES,
      }}
      defaultValues={{ destinationId: null, reason: "" }}
      toInput={toInput}
      successTitle={text.success}
      onClose={onClose}
    >
      <p>{text.intro}</p>
      <ChoiceInput
        name="destinationId"
        label="Destination"
        options={options}
        description={
          destinations.isError
            ? "The destinations could not be loaded."
            : "Verified destinations of the ordering client only."
        }
        isRequired
      />
      <ReasonField />
    </CommandDialog>
  );
}
