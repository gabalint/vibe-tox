import { Alert } from "@/components/Alert";
import { ErrorState } from "@/components/ErrorState";
import type { AuditStream } from "@/generated/AuditStream";
import type { ChainFinding } from "@/generated/ChainFinding";
import type { ChainVerification } from "@/generated/ChainVerification";
import type { VerifyAuditChainInput } from "@/generated/VerifyAuditChainInput";
import { useCommandMutation } from "@/lib/commands";
import styles from "./audit.module.css";
import { streamLabel } from "./streams";

/** Plain-language finding kinds from the engine's chain verification; others show as given. */
const FINDINGS: Readonly<Record<string, string>> = {
  hash_mismatch: "An event does not match its recorded hash: it was edited, moved or forged",
  broken_link:
    "An event does not link to the event before it: an earlier event was removed, inserted or moved",
  missing_events: "Events are missing: they were deleted",
  unknown_algorithm: "An event uses a hash algorithm this version cannot check",
  seal_mismatch: "A seal does not match the events it covers: events were changed after sealing",
};

/** Recomputes one stream's hash chain on request (REQ-OPS-001, NFR-18). */
export function useVerifyChain() {
  return useCommandMutation<ChainVerification, { input: VerifyAuditChainInput }>(
    "verify_audit_chain",
    { invalidates: [] },
  );
}

interface ChainResultProps {
  verify: ReturnType<typeof useVerifyChain>;
  /** The stream shown; a check of another stream is not shown beside its events. */
  stream: AuditStream;
}

/**
 * Whether the checked stream's chain is intact or what was found, under the page header so
 * it never pushes the events out of view while unused (UI review finding 22).
 */
export function ChainResult({ verify, stream }: ChainResultProps) {
  if (verify.variables?.input.stream !== stream) return null;
  if (verify.isError) {
    return <ErrorState title="The chain could not be verified" error={verify.error} />;
  }
  return verify.data ? <VerificationResult result={verify.data} /> : null;
}

function VerificationResult({ result }: { result: ChainVerification }) {
  const stream = streamLabel(result.stream).toLowerCase();
  if (result.intact) {
    return (
      <Alert tone="success" title="Chain intact">
        {result.eventsChecked} events in the {stream} stream were checked and no problem was found.
        A check on this computer cannot detect a chain rewritten from the point of change, or events
        removed from its end.
      </Alert>
    );
  }
  return (
    <Alert tone="danger" title="The chain is not intact">
      <p>
        {result.eventsChecked} events in the {stream} stream were checked. Problems found:
      </p>
      <ul className={styles.findings}>
        {result.findings.map((finding) => (
          <li key={`${finding.kind}-${finding.seq}`}>{findingText(finding)}</li>
        ))}
      </ul>
      <p>Keep this result and report it according to the laboratory's procedure.</p>
    </Alert>
  );
}

function findingText({ kind, seq }: ChainFinding): string {
  const text = FINDINGS[kind] ?? kind;
  return seq === null ? `${text}.` : `${text} (event ${seq}).`;
}
