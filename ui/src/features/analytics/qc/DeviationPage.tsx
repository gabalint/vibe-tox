import { useState } from "react";
import { useNavigate, useParams } from "react-router";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Timestamp } from "@/components/Timestamp";
import type { DeviationView } from "@/generated/DeviationView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import { CloseDeviationDialog } from "./CloseDeviationDialog";

/** Exact queue target for a deviation of any scope, including a shared lot. */
export function DeviationPage() {
  const navigate = useNavigate();
  const { deviationId = "" } = useParams();
  const query = useCommandQuery<DeviationView>("get_deviation", { input: { deviationId } });
  const canClose = useHasCapability("manage_quality");
  const person = usePersonText();
  const [closing, setClosing] = useState(false);
  return (
    <Page>
      <PageHeader
        title="Quality deviation"
        breadcrumbs={[{ label: "Work queues", href: "/work-queues" }]}
        actions={<RefreshControl query={query} />}
      />
      <QueryContent query={query} what="deviation">
        {(d) => (
          <>
            <Facts
              items={[
                { label: "Description", value: d.description },
                { label: "Scope", value: `${d.scope}: ${d.scopeId}` },
                { label: "Reason", value: d.reason },
                { label: "State", value: d.state },
                {
                  label: "Opened",
                  value: (
                    <>
                      <Timestamp value={d.openedAt} /> by {person(d.openedBy, d.openedByDisplay)}
                    </>
                  ),
                },
                { label: "Disposition", value: d.disposition ?? "Still open" },
                { label: "Closure reason", value: d.closeReason ?? "—" },
                {
                  label: "Closed",
                  value: d.closedAt ? (
                    <>
                      <Timestamp value={d.closedAt} /> by{" "}
                      {d.closedBy ? person(d.closedBy, d.closedByDisplay) : "Not recorded"}
                    </>
                  ) : (
                    "—"
                  ),
                },
              ]}
            />
            {d.scope === "run" && (
              <Button
                onPress={() =>
                  navigate(`/bench/runs/${d.scopeId}/qc/deviations?open=${d.deviationId}`)
                }
              >
                Open run quality context
              </Button>
            )}
            {canClose && d.state === "open" && (
              <Button onPress={() => setClosing(true)}>Close deviation</Button>
            )}
            {closing && (
              <CloseDeviationDialog
                deviation={d}
                onClose={() => {
                  setClosing(false);
                  void query.refetch();
                }}
              />
            )}
          </>
        )}
      </QueryContent>
    </Page>
  );
}
