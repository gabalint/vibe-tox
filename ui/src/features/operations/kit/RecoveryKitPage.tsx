import { useState } from "react";
import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { Facts } from "@/components/Facts";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { KeyState } from "@/generated/KeyState";
import type { RecoveryKitStatus } from "@/generated/RecoveryKitStatus";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { BACKUP_RESTORE, GET_RECOVERY_KIT_STATUS } from "../contract";
import type { Status } from "../labels";
import { Person } from "../Person";
import { ConfirmStorageDialog } from "./ConfirmStorageDialog";
import { ExportKitDialog } from "./ExportKitDialog";

// The engine sends the key state and the outdated reasons as codes; an unknown one shows as sent.
const KEY_STATES: Record<string, Status> & Record<KeyState, Status> = {
  ok: { label: "Key available", tone: "neutral" },
  missing: { label: "Key missing", tone: "danger" },
  mismatch: { label: "Key does not match this database", tone: "danger" },
  unavailable: { label: "Key store unavailable", tone: "danger" },
};

const OUTDATED_REASONS: Record<string, string> = {
  never_exported: "The recovery kit has never been exported.",
  key_changed: "The installation key changed since the last export.",
  targets_changed: "Backup targets were added or retired since the last export.",
};

/**
 * The recovery kit (REQ-OPS-002, design brief §3.3 and §3.9): the file holding the
 * installation key, which restores sign-in and backups on a replacement computer. The engine
 * says when the kit is outdated and why; this screen shows its answer.
 */
export function RecoveryKitPage() {
  const query = useCommandQuery<RecoveryKitStatus>(GET_RECOVERY_KIT_STATUS);
  const canAct = useHasCapability(BACKUP_RESTORE);
  const [dialog, setDialog] = useState<"export" | "confirm" | null>(null);
  const close = () => setDialog(null);
  return (
    <Page>
      <PageHeader
        title="Recovery kit"
        actions={
          canAct &&
          query.isSuccess && (
            <>
              <Button variant="primary" onPress={() => setDialog("export")}>
                Export recovery kit
              </Button>
              {query.data.lastExport && (
                <Button onPress={() => setDialog("confirm")}>
                  Confirm where the kit is stored
                </Button>
              )}
            </>
          )
        }
        description="The file that restores this installation's backups and sign-in on a replacement computer."
      />
      <QueryContent query={query} what="recovery kit status">
        {(status) => <KitStatus status={status} />}
      </QueryContent>
      {dialog === "export" && <ExportKitDialog onClose={close} />}
      {dialog === "confirm" && query.data?.lastExport && (
        <ConfirmStorageDialog kitNo={query.data.lastExport.kitNo} onClose={close} />
      )}
    </Page>
  );
}

function KitStatus({ status }: { status: RecoveryKitStatus }) {
  const keyState: Status = KEY_STATES[status.keyState] ?? {
    label: status.keyState,
    tone: "neutral",
  };
  const { lastExport, lastConfirmation } = status;
  return (
    <div className={styles.stack}>
      {status.outdated && (
        <Alert tone="warning" title="The recovery kit is outdated">
          <ul className={styles.list}>
            {status.outdatedReasons.map((reason) => (
              <li key={reason}>{OUTDATED_REASONS[reason] ?? reason}</li>
            ))}
          </ul>
        </Alert>
      )}
      <Section title="Installation key">
        <Facts
          layout="compact"
          items={[
            {
              label: "Key fingerprint",
              value: <span className={styles.code}>{status.keyFingerprint}</span>,
            },
            {
              label: "Key state",
              value: <StatusBadge tone={keyState.tone}>{keyState.label}</StatusBadge>,
            },
          ]}
        />
      </Section>
      <Section title="Last export">
        {lastExport ? (
          <Facts
            layout="compact"
            items={[
              { label: "Kit number", value: String(lastExport.kitNo) },
              { label: "Exported", value: <Timestamp value={lastExport.exportedAt} /> },
              {
                label: "Exported by",
                value: <Person id={lastExport.exportedBy} display={lastExport.exportedByDisplay} />,
              },
              { label: "File name", value: lastExport.fileName },
              {
                label: "Storage last confirmed",
                value: lastConfirmation ? (
                  <>
                    <Timestamp value={lastConfirmation.at} /> by{" "}
                    <Person id={lastConfirmation.by} display={lastConfirmation.byDisplay} />
                  </>
                ) : (
                  "Never confirmed"
                ),
              },
            ]}
          />
        ) : (
          <p>The recovery kit has never been exported.</p>
        )}
      </Section>
    </div>
  );
}
