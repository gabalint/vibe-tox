import { useFormContext, useWatch } from "react-hook-form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ContractEdge } from "@/generated/ContractEdge";
import type { DraftPartnerContract } from "@/generated/DraftPartnerContract";
import type { InterfaceContract } from "@/generated/InterfaceContract";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import { CommandDialog } from "@/lib/CommandDialog";
import { CONNECTION_QUERIES, DRAFT_CONTRACT } from "../../contract";
import { EDGES, PROFILE_EXAMPLES } from "./contractLabels";

interface ContractValues {
  profile: string;
  acknowledgements: string;
  corrections: string;
  edge: ContractEdge | "";
  engine: string;
  owners: string;
  deviations: string;
}

const EDGE_OPTIONS = Object.entries(EDGES).map(([id, label]) => ({ id, label }));

const toValues = (contract: InterfaceContract | null): ContractValues => ({
  profile: contract?.profile ?? "",
  acknowledgements: contract?.acknowledgements ?? "",
  corrections: contract?.corrections ?? "",
  edge: contract?.edge ?? "",
  engine: contract?.engine ?? "",
  owners: contract?.owners ?? "",
  deviations: contract?.deviations ?? "",
});

function toContract(values: ContractValues): InterfaceContract {
  const engine = values.edge === "lab_engine" ? values.engine.trim() : "";
  return {
    profile: values.profile.trim(),
    acknowledgements: values.acknowledgements.trim(),
    corrections: values.corrections.trim(),
    edge: values.edge as ContractEdge,
    ...(engine ? { engine } : {}),
    owners: values.owners.trim(),
    deviations: values.deviations.trim(),
  };
}

/** The interface engine's name, asked only when one sits between. */
function EngineInput() {
  const { control } = useFormContext<ContractValues>();
  if (useWatch({ control, name: "edge" }) !== "lab_engine") return null;
  return (
    <TextInput
      name="engine"
      label="Interface engine"
      description="Its name and version, such as the laboratory's engine 4.2"
      isRequired
    />
  );
}

/**
 * Drafts the connection's next interface contract version (REQ-INT-001): the terms the
 * partner's IT and the laboratory agreed, in their own words. It starts from the draft or the
 * version in force. Nothing changes until a second person approves it.
 */
export function DraftContractDialog({
  connectionId,
  messageType,
  from,
  next,
  onClose,
}: {
  connectionId: string;
  /** The connection's message type, for the profile's example. */
  messageType?: PartnerMessageType;
  /** The terms to start from: the draft, or the version in force; `null` for none. */
  from: InterfaceContract | null;
  /** The version the draft becomes once approved. */
  next: number;
  onClose: () => void;
}) {
  return (
    <CommandDialog<ContractValues, DraftPartnerContract, PartnerConnectionDetail>
      title={`Draft interface contract v${next}`}
      submitLabel="Save draft"
      command={{ name: DRAFT_CONTRACT, invalidates: CONNECTION_QUERIES }}
      defaultValues={toValues(from)}
      toInput={(values) => ({ connectionId, contract: toContract(values) })}
      successTitle={`Interface contract v${next} drafted`}
      fieldPaths={{ contract: "" }}
      size="wide"
      onClose={onClose}
    >
      <TextInput
        name="profile"
        label="Message and profile"
        description={PROFILE_EXAMPLES[messageType ?? "hl7_oru"]}
        isRequired
        autoFocus
      />
      <TextInput
        name="acknowledgements"
        label="Acknowledgements"
        description="What the partner's AA, AE and AR mean, or how it confirms a file"
        isRequired
      />
      <TextInput
        name="corrections"
        label="Corrections and withdrawals"
        description="How the partner applies a corrected result and a withdrawal"
        isRequired
      />
      <ChoiceInput
        name="edge"
        label="Between the LIS and the partner"
        options={EDGE_OPTIONS}
        isRequired
      />
      <EngineInput />
      <TextInput
        name="owners"
        label="Who answers for it"
        description="By role, at the partner and at the laboratory"
        isRequired
      />
      <TextInput
        name="deviations"
        label="Agreed deviations"
        description="From the profile; leave empty for none"
      />
    </CommandDialog>
  );
}
