import { useState } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import type { HealthCheckView } from "@/generated/HealthCheckView";

/** The parent keys this notice by version and warning identity to reset its local dismissal. */
export function VersionNotice({ version }: { version: HealthCheckView }) {
  const [dismissed, setDismissed] = useState(false);
  if (dismissed) return null;
  return (
    <Alert
      tone={version.state === "alert" ? "danger" : "warning"}
      title={version.label}
      onDismiss={() => setDismissed(true)}
    >
      {version.summary} <Link to="/operations/versions/installed">Versions and updates</Link>
    </Alert>
  );
}
