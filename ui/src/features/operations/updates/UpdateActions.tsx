import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import { UpdateDialog } from "./UpdateDialog";

/**
 * Updates to one newer build (ADR 0020; REQ-OPS-006, NFR-24), for people with
 * `manage_configuration`: one button, then one confirmation that downloads, backs up, approves
 * and installs. The app then restarts into the new version, which still needs acceptance for
 * clinical use.
 */
export function UpdateActions({ build }: { build: AvailableBuild }) {
  const [updating, setUpdating] = useState(false);
  const [installing, setInstalling] = useState(false);
  if (installing) {
    return (
      <Alert tone="info" title="Installing the update">
        The app closes and starts version {build.version}. Accept it for clinical use after the
        laboratory’s checks.
      </Alert>
    );
  }
  return (
    <>
      <Button variant="primary" onPress={() => setUpdating(true)}>
        Update to version {build.version}
      </Button>
      {updating && (
        <UpdateDialog
          build={build}
          onClose={() => setUpdating(false)}
          onInstalling={() => {
            setUpdating(false);
            setInstalling(true);
          }}
        />
      )}
    </>
  );
}
