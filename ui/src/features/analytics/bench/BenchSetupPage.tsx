import { useState } from "react";
import { Button } from "@/components/Button";
import { Page, PageHeader } from "@/components/Page";
import { Tabs } from "@/components/Tabs";
import { useHasCapability } from "@/lib/commands";
import { useRouteTabs } from "@/lib/routeTabs";
import { MANAGE_QUALITY } from "../contract";
import { InstrumentsTab } from "./InstrumentsTab";
import { LotsTab } from "./LotsTab";

/** One route per tab; `/bench/setup` shows the first. */
const TABS = [
  { id: "instruments", label: "Instruments" },
  { id: "lots", label: "Lots" },
] as const;

/**
 * What the bench needs before a run (REQ-ANA-007, REQ-ANA-003 registration part): each
 * instrument's service status with its approved result sources, and the lots in use. The
 * engine checks all three again when a run is created and at review.
 */
export function BenchSetupPage() {
  const { selectedId: tab, tabs } = useRouteTabs("/bench/setup", TABS);
  const canManage = useHasCapability(MANAGE_QUALITY);
  const [isRegistering, setRegistering] = useState(false);
  return (
    <Page>
      <PageHeader
        title="Bench setup"
        actions={
          tab === "lots" &&
          canManage && (
            <Button variant="primary" onPress={() => setRegistering(true)}>
              Register lot
            </Button>
          )
        }
      />
      <Tabs label="Bench setup" tabs={tabs} selectedId={tab} contentLayout="collection">
        {tab === "lots" ? (
          <LotsTab
            isRegistering={isRegistering}
            onCloseRegistration={() => setRegistering(false)}
          />
        ) : (
          <InstrumentsTab />
        )}
      </Tabs>
    </Page>
  );
}
