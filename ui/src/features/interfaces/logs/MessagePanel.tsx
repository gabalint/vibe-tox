import { useId } from "react";
import { Link } from "react-router";
import { Alert } from "@/components/Alert";
import { DetailPanel, useWidePanel } from "@/components/DetailPanel";
import { Facts } from "@/components/Facts";
import { QueryContent } from "@/components/QueryContent";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { MessageContent } from "@/generated/MessageContent";
import type { PartnerAttempt } from "@/generated/PartnerAttempt";
import type { PartnerMessageDetail } from "@/generated/PartnerMessageDetail";
import type { PartnerMessageRow } from "@/generated/PartnerMessageRow";
import { useCommandQuery } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import { GET_MESSAGE } from "../contract";
import { applicationOf, MESSAGE_TYPES, problemText, recordText, transportOf } from "../labels";
import { connectionPath } from "../paths";
import styles from "./MessagePanel.module.css";
import { MessageDetails } from "./messageColumns";

/** Why the message itself is not shown. */
const NOT_SHOWN: Record<Exclude<MessageContent, "sent">, string> = {
  not_made:
    "Not made yet. The message is made at its first attempt, once every test and analyte has an approved code mapping.",
  file: "This message is a file, not an HL7 message. Open it from the report or billing handoff it carries.",
  unavailable:
    "The copy kept when this message was made is missing or no longer matches its digest, so it is not shown. Tell your system administrator.",
};

interface MessagePanelProps {
  row: PartnerMessageRow;
  deliveryId: string;
  /** Whether the connection is linked; not on the connection's own Logs tab. */
  opensConnection: boolean;
  onClose: () => void;
  onPrevious: (() => void) | undefined;
  onNext: (() => void) | undefined;
}

/**
 * One outgoing message (ticket #4678): what became of it and why it waits, as in the Details
 * column the open panel pushes out of view (#4726), its control ID, the partner's answer, the HL7 message
 * exactly as sent and every attempt. The message is the copy kept when it was made, checked
 * against its digest by the engine, never made again. It holds patient data: it is read only
 * when the panel opens, and each read is recorded in the audit trail.
 */
export function MessagePanel({
  row,
  deliveryId,
  opensConnection,
  ...navigation
}: MessagePanelProps) {
  const detail = useCommandQuery<PartnerMessageDetail>(GET_MESSAGE, { input: { deliveryId } });
  useWidePanel(detail.data?.content === "sent");
  const formatTime = useFormatTime();
  const person = usePersonText();
  const transport = transportOf(row);
  return (
    <DetailPanel
      identifier={`${recordText(row)} – ${MESSAGE_TYPES[row.messageType]}`}
      status={<StatusBadge tone={transport.tone}>{transport.label}</StatusBadge>}
      {...navigation}
    >
      <QueryContent query={detail} what="message" loadingRows={4}>
        {(loaded) => (
          <>
            <Facts
              items={[
                {
                  label: "Connection",
                  value:
                    opensConnection && row.connection ? (
                      <Link to={connectionPath(row.connection.id)}>{row.connection.name}</Link>
                    ) : (
                      (row.connection?.name ?? "None matched")
                    ),
                },
                {
                  label: "Details",
                  value: <MessageDetails row={row} formatTime={formatTime} person={person} />,
                },
                { label: "Control ID (MSH-10)", value: loaded.controlId ?? "—" },
                { label: "Partner's answer", value: <Answer row={row} /> },
              ]}
            />
            <SentMessage detail={loaded} test={row.source === "test"} />
            <Attempts attempts={loaded.attempts} test={row.source === "test"} />
          </>
        )}
      </QueryContent>
    </DetailPanel>
  );
}

/** The partner's MSA code and text, or why there is none. */
function Answer({ row }: { row: PartnerMessageRow }) {
  const answer = applicationOf(row);
  if (row.connection?.protocol !== "mllp_connect") {
    return <>None: a folder or SFTP server never answers.</>;
  }
  if (row.ackCode !== null && answer) {
    return (
      <>
        <StatusBadge tone={answer.tone}>{answer.label}</StatusBadge>
        {row.ackText && <span className={styles.ackText}>{row.ackText}</span>}
      </>
    );
  }
  if (row.errorCode !== null) return <>No answer: {problemText(row.errorCode)}.</>;
  return <>None yet.</>;
}

function SentMessage({ detail, test }: { detail: PartnerMessageDetail; test: boolean }) {
  const id = useId();
  return (
    <section className={styles.part} aria-labelledby={id}>
      <div className={styles.heading}>
        <h3 id={id}>Message as sent</h3>
        {detail.content === "sent" && (
          <StatusBadge tone="success">Matches what was sent</StatusBadge>
        )}
      </div>
      {detail.content === "sent" ? (
        <>
          {test && <p className={styles.note}>Synthetic test message: no patient data.</p>}
          <pre className={styles.message}>{detail.segments.join("\n")}</pre>
        </>
      ) : detail.content === "unavailable" ? (
        <Alert tone="danger" compact>
          {NOT_SHOWN.unavailable}
        </Alert>
      ) : (
        <p className={styles.note}>{NOT_SHOWN[detail.content]}</p>
      )}
    </section>
  );
}

function Attempts({ attempts, test }: { attempts: readonly PartnerAttempt[]; test: boolean }) {
  const id = useId();
  const none = test
    ? "Sent once when the test ran; a test message is never sent again."
    : "None yet.";
  return (
    <section className={styles.part} aria-labelledby={id}>
      <h3 id={id}>Attempts</h3>
      {attempts.length === 0 ? (
        <p className={styles.note}>{none}</p>
      ) : (
        <ol className={styles.attempts}>
          {attempts.map((attempt, index) => (
            // Attempts never change order: the audit trail is append-only.
            // biome-ignore lint/suspicious/noArrayIndexKey: two attempts may share a time.
            <li key={index}>
              <Timestamp value={attempt.at} /> – {attemptText(attempt)}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function attemptText({ succeeded, errorCode, ackCode }: PartnerAttempt): string {
  if (!succeeded)
    return `Failed: ${errorCode === null ? "no reason recorded" : problemText(errorCode)}`;
  // Only an MLLP partner answers; a folder or SFTP server is written to.
  return ackCode === null ? "Written" : `Sent; the partner answered ${ackCode}`;
}
