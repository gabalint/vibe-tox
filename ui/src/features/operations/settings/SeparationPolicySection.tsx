import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { Timestamp } from "@/components/Timestamp";
import type { SeparationPolicyView } from "@/generated/SeparationPolicyView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { APPROVE_CONFIGURATION, GET_SEPARATION_POLICY } from "../contract";
import { Person } from "../Person";
import { SeparationPolicyDialog } from "./SeparationPolicyDialog";
import { DUTIES } from "./separationDuties";

/**
 * The separation-of-duties policy in force (ADR 0018), editable by the administrator
 * and readable by quality: for each duty, whether one person may do both steps, and who set the
 * version and why. Until the laboratory sets one, a second person is required for each.
 */
export function SeparationPolicySection() {
  const view = useCommandQuery<SeparationPolicyView>(GET_SEPARATION_POLICY);
  const canApprove = useHasCapability(APPROVE_CONFIGURATION);
  const [isOpen, setOpen] = useState(false);
  return (
    <Section
      title="Separation of duties"
      actions={
        view.data &&
        canApprove && <Button onPress={() => setOpen(true)}>Change separation of duties</Button>
      }
    >
      <p>
        Whether the person who did the first step of a duty may also do the second. US clinical
        laboratory rules let one qualified person hold every role, so this is the laboratory's
        decision; its procedures, accreditor or state may still require a second person. Each
        self-approval needs the person's confirmation every time and is marked in the audit trail.
      </p>
      <QueryContent query={view} what="separation of duties">
        {(data) => <PolicyFacts view={data} />}
      </QueryContent>
      {isOpen && view.data && canApprove && (
        <SeparationPolicyDialog view={view.data} onClose={() => setOpen(false)} />
      )}
    </Section>
  );
}

function PolicyFacts({ view }: { view: SeparationPolicyView }) {
  const duties = DUTIES.map(({ id, label }) => ({
    label,
    value: view.values[id]
      ? "One person may do both, confirming each time"
      : "Second person required",
  }));
  if (view.version === null) {
    return (
      <>
        <Alert tone="info" title="Default applies">
          Default: a second person is required for each.
        </Alert>
        <Facts items={duties} />
      </>
    );
  }
  return (
    <Facts
      items={[
        ...duties,
        { label: "Version", value: String(view.version) },
        { label: "Reason", value: view.reason ?? "Not recorded" },
        {
          label: "Set",
          value: (
            <>
              {view.setAt ? <Timestamp value={view.setAt} /> : "Unknown time"} by{" "}
              {view.setBy ? <Person id={view.setBy} display={view.setByDisplay} /> : "unknown"}
            </>
          ),
        },
      ]}
    />
  );
}
