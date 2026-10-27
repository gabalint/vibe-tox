import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Toolbar } from "@/components/Toolbar";
import type { AvailableBuild } from "@/generated/AvailableBuild";
import type { DownloadedUpdateView } from "@/generated/DownloadedUpdateView";
import type { DownloadUpdateInput } from "@/generated/DownloadUpdateInput";
import type { InstallUpdateInput } from "@/generated/InstallUpdateInput";
import type { UpdateApprovalView } from "@/generated/UpdateApprovalView";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import {
  DOWNLOAD_UPDATE,
  INSTALL_UPDATE,
  LIST_UPDATE_APPROVALS,
  VERSION_QUERIES,
} from "../contract";
import { RecordApprovalDialog } from "./RecordApprovalDialog";

/**
 * Downloads, approves and installs one newer build (ADR 0020; REQ-OPS-006, NFR-24), for people
 * with `manage_configuration`. The app verifies the package's signature as it downloads; the
 * engine records the approval of that exact package with a verified pre-update backup, and
 * allows the installation only under it. The app then restarts into the new version, which
 * still needs acceptance for clinical use.
 */
export function UpdateActions({ build }: { build: AvailableBuild }) {
  const [approving, setApproving] = useState(false);
  const download = useCommandMutation<DownloadedUpdateView, { input: DownloadUpdateInput }>(
    DOWNLOAD_UPDATE,
    { invalidates: [] },
  );
  const install = useCommandMutation<null, { input: InstallUpdateInput }>(INSTALL_UPDATE, {
    invalidates: VERSION_QUERIES,
  });
  const approvals = useCommandQuery<UpdateApprovalView[]>(LIST_UPDATE_APPROVALS);
  const downloaded = download.data?.version === build.version ? download.data : undefined;
  // A hint only: the engine compares the package's SHA-256 before it allows installation.
  const approved = approvals.data?.some(
    (approval) =>
      approval.version === build.version &&
      !approval.withdrawn &&
      !approval.installed &&
      approval.packageFileName === downloaded?.packageFileName,
  );
  const installNow = () => install.mutate({ input: { version: build.version } });
  if (install.isSuccess) {
    return (
      <Alert tone="info" title="Installing the update">
        The app closes and starts version {build.version}. Accept it for clinical use after the
        laboratory’s checks.
      </Alert>
    );
  }
  return (
    <>
      <Toolbar>
        {!downloaded && (
          <Button
            variant="primary"
            isPending={download.isPending}
            onPress={() =>
              download.mutate({ input: { channel: build.channel, version: build.version } })
            }
          >
            Download version {build.version}
          </Button>
        )}
        {downloaded && !approved && (
          <Button
            variant="primary"
            isPending={install.isPending}
            onPress={() => setApproving(true)}
          >
            Approve and install
          </Button>
        )}
        {downloaded && approved && (
          <Button variant="primary" isPending={install.isPending} onPress={installNow}>
            Install approved update
          </Button>
        )}
      </Toolbar>
      {download.isPending && (
        <Alert tone="info">Downloading version {build.version} and checking its signature…</Alert>
      )}
      {downloaded && !install.isPending && (
        <Alert tone="success">
          Downloaded {downloaded.packageFileName}; its signature is verified.
        </Alert>
      )}
      {install.isPending && <Alert tone="info">Installing version {build.version}…</Alert>}
      {download.error && <ErrorState title="Not downloaded" error={download.error} />}
      {install.error && <ErrorState title="Not installed" error={install.error} />}
      {approving && downloaded && (
        <RecordApprovalDialog
          downloaded={downloaded}
          onApproved={installNow}
          onClose={() => setApproving(false)}
        />
      )}
    </>
  );
}
