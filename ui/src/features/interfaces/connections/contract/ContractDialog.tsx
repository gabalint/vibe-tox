import { useState } from "react";
import { Button } from "@/components/Button";
import { Dialog } from "@/components/Dialog";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { InterfaceContract } from "@/generated/InterfaceContract";
import type { PartnerConnectionDetail } from "@/generated/PartnerConnectionDetail";
import type { PartnerContract } from "@/generated/PartnerContract";
import { useCommandMutation } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import row from "../../actionRow.module.css";
import { CONNECTION_QUERIES, RUN_ACCEPTANCE } from "../../contract";
import dialog from "../ConnectionDialog.module.css";
import { ApproveContractDialog } from "./ApproveContractDialog";
import styles from "./ContractDialog.module.css";
import { ACCEPTANCE, caseText, EDGES, problemText } from "./contractLabels";
import { DraftContractDialog } from "./DraftContractDialog";

/** The agreed terms, as recorded. */
function terms(contract: InterfaceContract) {
  const edge =
    contract.edge === "lab_engine" && contract.engine
      ? `${EDGES.lab_engine}: ${contract.engine}`
      : EDGES[contract.edge];
  return [
    { label: "Message and profile", value: contract.profile },
    { label: "Acknowledgements", value: contract.acknowledgements },
    { label: "Corrections and withdrawals", value: contract.corrections },
    { label: "Between the LIS and the partner", value: edge },
    { label: "Who answers for it", value: contract.owners },
    { label: "Agreed deviations", value: contract.deviations || "None" },
  ];
}

/** The latest run: its state, when and by whom, and each case with why it failed. */
function Acceptance({ contract }: { contract: PartnerContract }) {
  const formatTime = useFormatTime();
  const person = usePersonText();
  const state = ACCEPTANCE[contract.acceptance];
  const run = contract.lastRun;
  return (
    <Section title="Acceptance tests">
      <p>
        <StatusBadge tone={state.tone}>{state.label}</StatusBadge>
        {run && ` Last run by ${person(run.ran.by, run.ran.byDisplay)}, ${formatTime(run.ran.at)}.`}
      </p>
      {run && (
        <ul className={styles.cases}>
          {run.cases.map((item) => (
            <li key={item.code} className={item.passed ? row.ok : row.problem}>
              {item.passed ? "✓" : "✗"} {caseText(item.code)}
              {item.problem && `: ${problemText(item.problem)}`}
            </li>
          ))}
        </ul>
      )}
      <p className={dialog.hint}>
        Synthetic messages through the connection's settings and approved code mappings. Nothing is
        sent, and nothing they make is kept.
      </p>
    </Section>
  );
}

type Open = "draft" | "approve" | null;

/**
 * The connection's interface contract (REQ-INT-001): the version in force, the draft waiting
 * for approval, and the acceptance tests for the connection as it is now. Patient data goes
 * or comes only once a version is approved and the latest tests passed.
 */
export function ContractDialog({
  detail,
  onClose,
}: {
  detail: PartnerConnectionDetail;
  onClose: () => void;
}) {
  const [open, setOpen] = useState<Open>(null);
  const formatTime = useFormatTime();
  const person = usePersonText();
  const { connection, contract } = detail;
  const run = useCommandMutation<PartnerConnectionDetail, { input: { connectionId: string } }>(
    RUN_ACCEPTANCE,
    { invalidates: CONNECTION_QUERIES },
  );
  const allowed = (action: string) => contract.allowedActions.includes(action);
  const next = (contract.current?.version ?? 0) + 1;
  const back = () => setOpen(null);
  if (open === "draft") {
    const from = contract.draft?.contract ?? contract.current?.contract ?? null;
    return (
      <DraftContractDialog
        connectionId={connection.id}
        messageType={connection.settings.messageType}
        from={from}
        next={next}
        onClose={back}
      />
    );
  }
  if (open === "approve") {
    return (
      <ApproveContractDialog
        connectionId={connection.id}
        expectedRevision={connection.revision}
        next={next}
        onClose={back}
      />
    );
  }
  const { current, draft } = contract;
  return (
    <Dialog
      title="Interface contract"
      description="What the partner and the laboratory agreed, and whether the LIS's tests pass."
      isOpen
      onOpenChange={(isOpen) => !isOpen && onClose()}
      size="wide"
      actions={
        <>
          {allowed(RUN_ACCEPTANCE) && (
            <Button
              isPending={run.isPending}
              onPress={() => run.mutate({ input: { connectionId: connection.id } })}
            >
              Run acceptance tests
            </Button>
          )}
          {allowed("approve_partner_contract") && (
            <Button onPress={() => setOpen("approve")}>Approve v{next}</Button>
          )}
          {allowed("draft_partner_contract") && (
            <Button onPress={() => setOpen("draft")}>
              {draft ? `Change draft v${next}` : `Draft v${next}`}
            </Button>
          )}
          <Button variant="primary" onPress={onClose}>
            Close
          </Button>
        </>
      }
    >
      <Section title={current ? `In force: v${current.version}` : "No version approved yet"}>
        {current ? (
          <Facts
            items={[
              ...terms(current.contract),
              {
                label: "Approved",
                value: `${person(current.approved.by, current.approved.byDisplay)}, ${formatTime(current.approved.at)} · reason: ${current.reason}`,
              },
            ]}
          />
        ) : (
          <p>No patient data goes or comes until a second person approves a version.</p>
        )}
      </Section>
      {draft && (
        <Section title={`Draft v${next}, waiting for approval`}>
          <Facts
            items={[
              ...terms(draft.contract),
              {
                label: "Drafted",
                value: `${person(draft.drafted.by, draft.drafted.byDisplay)}, ${formatTime(draft.drafted.at)}`,
              },
            ]}
          />
        </Section>
      )}
      <Acceptance contract={contract} />
      {run.error && <ErrorState title="Tests not run" error={run.error} />}
    </Dialog>
  );
}
