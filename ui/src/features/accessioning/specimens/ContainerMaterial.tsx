import { useState } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { ContainerView } from "@/generated/ContainerView";
import { useHasCapability } from "@/lib/commands";
import layout from "@/styles/layout.module.css";
import { type MaterialAction, MaterialActionDialog } from "./MaterialActionDialog";
import { MaterialHistory } from "./MaterialHistory";
export function ContainerMaterial({ container }: { container: ContainerView }) {
  const [action, setAction] = useState<MaterialAction | null>(null);
  const accession = useHasCapability("accession");
  const quality = useHasCapability("manage_quality");
  const allowed = (name: string) => container.allowedActions.includes(name);
  return (
    <section aria-label={`Material actions for ${container.labelCode}`}>
      {container.locationChronologyUnresolved && (
        <Alert tone="warning" title="Current location needs source review">
          Legacy moves cannot establish the current location. Check the physical container and
          record an attributable move to its verified location.
        </Alert>
      )}
      <Toolbar>
        {accession && allowed("move_container") && (
          <Button onPress={() => setAction("move")}>Move container</Button>
        )}
        {quality && container.state !== "disposed" && (
          <Button onPress={() => setAction("hold")}>Place hold</Button>
        )}
        {accession && allowed("mark_container_depleted") && (
          <Button onPress={() => setAction("deplete")}>Mark depleted</Button>
        )}
        {accession && allowed("dispose_container") && (
          <Button onPress={() => setAction("dispose")}>Dispose container</Button>
        )}
        <Link to="/specimens/storage">Storage excursions</Link>
      </Toolbar>
      <h3>Open holds</h3>
      {container.openHolds.length === 0 ? (
        <p>No open holds.</p>
      ) : (
        <ul className={layout.list}>
          {container.openHolds.map((hold) => (
            <li key={hold.holdId}>
              <Toolbar>
                <span>
                  {hold.kind}: {hold.reason}. Placed <Timestamp value={hold.placedAt} />.
                </span>
                {hold.excursionId && (
                  <Link to={`/specimens/storage?excursionId=${hold.excursionId}`}>
                    View excursion
                  </Link>
                )}
                {quality && (
                  <Button onPress={() => setAction(hold)}>Release {hold.kind} hold</Button>
                )}
              </Toolbar>
            </li>
          ))}
        </ul>
      )}
      <h3>Label job history</h3>
      {container.labelJobs.length === 0 ? (
        <p>No labels requested.</p>
      ) : (
        <ul>
          {container.labelJobs.map((job) => (
            <li key={job.jobId}>
              {job.kind}: {job.outcome ?? "queued"}. <Timestamp value={job.requestedAt} />
              {job.errorCode && ` — ${job.errorCode}`}
            </li>
          ))}
        </ul>
      )}
      <p>
        A queued or handed-off job does not prove a label printed. Refresh after interruption; check
        the physical printer and label before requesting a reasoned reprint of the same barcode.
      </p>
      <MaterialHistory containerId={container.containerId} />
      {action && (
        <MaterialActionDialog
          container={container}
          action={action}
          onClose={() => setAction(null)}
        />
      )}
    </section>
  );
}
