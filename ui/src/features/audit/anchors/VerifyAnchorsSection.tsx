import { Alert } from "@/components/Alert";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Toolbar } from "@/components/Toolbar";
import type { AnchorVerification } from "@/generated/AnchorVerification";
import type { VerifyAuditAnchorsInput } from "@/generated/VerifyAuditAnchorsInput";
import { useCommandMutation } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { anchorFindingText, COMPARISON_ROWS, streamText } from "./anchorText";
import { VERIFY_AUDIT_ANCHORS } from "./contract";

/**
 * Compares the kept anchors with this computer's audit trail (NFR-18), from every available
 * target or from a folder chosen in the OS dialog. The engine records each verification in the
 * audit trail. With no available target, or a folder it cannot read, the engine refuses with
 * `conflict` and its message is shown.
 */
export function VerifyAnchorsSection() {
  const verify = useCommandMutation<AnchorVerification | null, { input: VerifyAuditAnchorsInput }>(
    VERIFY_AUDIT_ANCHORS,
    { invalidates: [] },
  );
  const run = (chooseFolder: boolean) => verify.mutate({ input: { chooseFolder } });
  return (
    <div className={styles.stack}>
      <p>
        Checks the audit trail against the anchors kept on the backup targets, or against anchors
        copied to another folder. Each verification is recorded in the audit trail.
      </p>
      <Toolbar>
        <Button isPending={verify.isPending} onPress={() => run(false)}>
          Verify anchors
        </Button>
        <Button isDisabled={verify.isPending} onPress={() => run(true)}>
          Verify with anchors from another folder
        </Button>
      </Toolbar>
      {verify.error && <ErrorState title="Anchors not verified" error={verify.error} />}
      {verify.isSuccess &&
        (verify.data === null ? (
          <Alert tone="info">No folder was chosen, so nothing was verified.</Alert>
        ) : (
          <VerificationResult result={verify.data} />
        ))}
    </div>
  );
}

function VerificationResult({ result }: { result: AnchorVerification }) {
  return (
    <div className={styles.stack}>
      <Facts
        layout="compact"
        items={[
          { label: "Anchors read", value: String(result.anchorsRead) },
          ...COMPARISON_ROWS.map(([key, label]) => ({ label, value: String(result.summary[key]) })),
          ...result.chains.map(({ stream, intact }) => ({
            label: `${streamText(stream)} chain`,
            value: (
              <StatusBadge tone={intact ? "success" : "danger"}>
                {intact ? "Intact" : "Not intact"}
              </StatusBadge>
            ),
          })),
        ]}
      />
      <h3 className={styles.subheading}>Findings</h3>
      {result.findings.length === 0 ? (
        <p>No findings.</p>
      ) : (
        <ul className={styles.list}>
          {result.findings.map((finding) => (
            <li key={`${finding.code}-${finding.stream}-${finding.sealNo}`}>
              <StatusBadge tone={finding.problem ? "danger" : "info"}>
                {finding.problem ? "Needs investigation" : "For information"}
              </StatusBadge>{" "}
              {anchorFindingText(finding)}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
