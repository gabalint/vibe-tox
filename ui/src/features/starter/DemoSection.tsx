import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { DemoDataLoaded } from "@/generated/DemoDataLoaded";
import type { DemoStatus } from "@/generated/DemoStatus";
import { CommandRefusal } from "@/lib/CommandRefusal";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { DEMO_INVALIDATES, DEMO_STATUS, LOAD_DEMO_DATA } from "./contract";

/**
 * Demo data (proposed REQ-OPS-016), shown only in the demo laboratory: fictitious clients,
 * practitioners, delivery destinations and patients from the file shipped with the app. A
 * laboratory installation shows nothing here, and its engine refuses the load anyway.
 */
export function DemoSection() {
  const status = useCommandQuery<DemoStatus>(DEMO_STATUS);
  const [confirming, setConfirming] = useState(false);
  const [result, setResult] = useState<DemoDataLoaded | null>(null);
  const load = useCommandMutation<DemoDataLoaded>(LOAD_DEMO_DATA, {
    invalidates: DEMO_INVALIDATES,
  });
  if (!status.data?.isDemo) return null;
  const run = () =>
    load.mutate(
      {},
      {
        onSuccess: (loaded) => {
          setResult(loaded);
          setConfirming(false);
        },
      },
    );
  return (
    <Section
      title="Demo data"
      actions={<Button onPress={() => setConfirming(true)}>Load Demo Data</Button>}
    >
      <p>
        Fictitious clients, practitioners, delivery destinations and patients for exploring the app.
        Loading needs the Administrator and Accessioner roles. Load the Standard Catalog first,
        signed in as the same person, so that demo values fill its blank cutoffs.
      </p>
      {status.data.demoDataLoaded && !result && <p>Demo data is loaded.</p>}
      {result && <DemoResult result={result} />}
      <ConfirmDialog
        title="Load demo data?"
        isOpen={confirming}
        onOpenChange={setConfirming}
        confirmLabel="Load Demo Data"
        variant="primary"
        onConfirm={run}
        isPending={load.isPending}
      >
        <div className={layout.stack}>
          <p>
            Adds the demo file's fictitious parties and patients to this demo laboratory. They
            cannot be removed, except by resetting the demo laboratory. Loading again adds nothing.
          </p>
          {load.error && <CommandRefusal title="Not loaded" error={load.error} lead={null} />}
        </div>
      </ConfirmDialog>
    </Section>
  );
}

function DemoResult({ result }: { result: DemoDataLoaded }) {
  return (
    <Alert tone="success" title="Demo data loaded">
      <p>{result.notice}</p>
      <Facts
        layout="compact"
        items={[
          {
            label: "Laboratory profile",
            value: result.laboratoryProfileSet ? "Set to the demo profile" : "Kept as it was",
          },
          { label: "Clients", value: String(result.clients) },
          { label: "Client locations", value: String(result.clientLocations) },
          { label: "Delivery destinations", value: String(result.deliveryDestinations) },
          { label: "Practitioners", value: String(result.practitioners) },
          { label: "Patients", value: String(result.patients) },
          { label: "Identity holds opened by design", value: String(result.identityHolds) },
          {
            label: "Blank cutoffs filled with demo values",
            value: String(result.demoCutoffsFilled),
          },
          { label: "Blank cutoffs not filled", value: String(result.demoCutoffsNotFilled) },
        ]}
      />
      {result.demoCutoffsNotFilled > 0 && (
        <p>
          Some blank cutoffs were not filled: another person loaded those drafts, and only a draft's
          author may change it.
        </p>
      )}
    </Alert>
  );
}
