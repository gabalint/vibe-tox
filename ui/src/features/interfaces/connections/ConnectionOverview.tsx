import { Fragment, type ReactNode } from "react";
import { Section } from "@/components/Section";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import {
  messageText,
  noRulesText,
  PROTOCOLS,
  problemText,
  ruleText,
  SEND_WHEN,
  scheduleText,
  shortFingerprint,
} from "../labels";
import { listeningText, socketText } from "../mllpLabels";
import styles from "./ConnectionPage.module.css";
import { partnersText } from "./listenerForm";
import { CodeMappingsSection } from "./mappings/CodeMappingsSection";

/** A titled card of label–value pairs. */
function Card({ title, items }: { title: string; items: [string, ReactNode][] }) {
  return (
    <Section title={title}>
      <dl className={styles.facts}>
        {items.map(([label, value]) => (
          <Fragment key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </Fragment>
        ))}
      </dl>
    </Section>
  );
}

const code = (key: string, text: string) => (
  <span key={key} className={styles.code}>
    {text}
  </span>
);

/**
 * Where it writes, as in the mockup: the protocol and folder; for SFTP the protocol with the
 * server, the remote folder and the sign-in with the server's fingerprint; for MLLP the
 * partner's address and the acknowledgement the LIS waits for, or where the LIS listens, for
 * whom, and whether it listens now.
 */
function destinationItems(
  { folder, settings, listener }: PartnerConnection,
  formatTime: (at: string) => string,
): [string, ReactNode][] {
  const { sftp, mllp, mllpListener } = settings;
  if (mllpListener) {
    return [
      ["Protocol", `${PROTOCOLS.mllp_listen} – ${socketText(mllpListener)}`],
      ["Partners allowed", partnersText(mllpListener.allowedPartners)],
      [
        "Acknowledgement",
        `Application ACK (AA / AE / AR), after the message is saved · idle timeout ${mllpListener.idleTimeoutSeconds} seconds`,
      ],
      ["Listener", listeningText(listener, formatTime)],
    ];
  }
  if (mllp) {
    return [
      ["Protocol", `${PROTOCOLS.mllp_connect} – ${socketText(mllp)}`],
      [
        "Acknowledgement",
        `Application ACK (AA / AE / AR) · timeout ${mllp.ackTimeoutSeconds} seconds`,
      ],
    ];
  }
  const input = settings.intake ? "Input folder" : "Folder";
  if (sftp === null) {
    return [
      ["Protocol", PROTOCOLS[settings.protocol]],
      [input, code("folder", folder)],
    ];
  }
  const signIn = sftp.auth === "key" ? "SSH key" : "Password";
  return [
    ["Protocol", `SFTP – ${sftp.host}:${sftp.port}`],
    [`Remote ${input.toLowerCase()}`, code("remote-folder", sftp.remoteFolder)],
    [
      "Sign-in",
      `${signIn} as ${sftp.username} · fingerprint ${shortFingerprint(sftp.fingerprint)}`,
    ],
  ];
}

/** When and what: sending's schedule, file name and rules, or reading's folders and rules. */
function workingItems(connection: PartnerConnection): [string, ReactNode][] {
  const { settings } = connection;
  const { intake } = settings;
  if (intake) {
    // A listener receives each message as it comes, with nothing to move.
    const files: [string, ReactNode][] = settings.mllpListener
      ? []
      : [
          ["Processed folder", code("processed", connection.processedFolder ?? "")],
          ["Failed folder", code("failed", connection.failedFolder ?? "")],
          ["Check every", `${intake.checkEverySeconds} seconds`],
        ];
    return [
      ...files,
      ["Accept messages from", `MSH-4 = ${intake.acceptFrom}`],
      ["Unknown patient", unknownPatientText(connection)],
    ];
  }
  const type = settings.messageType;
  const fileName: [string, ReactNode][] = settings.mllp
    ? []
    : [["File name", code("file-name", settings.fileNamePattern)]];
  return [
    ["Send when", `${SEND_WHEN[type]} · ${scheduleText(settings.schedule).toLowerCase()}`],
    ...fileName,
    ["Rules", rulesText(connection)],
  ];
}

/** What a refused order becomes: its file moves to Failed; over MLLP the partner reads AE. */
const refusedText = ({ settings }: PartnerConnection) =>
  settings.mllpListener ? "Rejected with AE and logged" : "Moved to Failed and logged";

const unknownPatientText = (connection: PartnerConnection) =>
  connection.settings.intake?.unknownPatient === "hold"
    ? "Held until the patient is registered"
    : refusedText(connection);

const rulesText = (connection: PartnerConnection) =>
  connection.settings.rules.length === 0
    ? "None: every message of this type"
    : connection.settings.rules
        .map((rule) => ruleText(rule, connection.settings.messageType))
        .join("; ");

interface OverviewProps {
  connection: PartnerConnection;
  /** The reason given for the latest change, if any. */
  lastReason: string | null;
}

/**
 * Overview, as in the mockup: the connection's settings beside its checks — who verified the
 * destination and how, the last attempt and its result, what waits for an acknowledgement and
 * the last change with its reason.
 */
export function ConnectionOverview({ connection, lastReason }: OverviewProps) {
  const formatTime = useFormatTime();
  const person = usePersonText();
  const { settings, verification, lastAttempt, lastChange } = connection;
  const type = settings.messageType;
  return (
    <div className={styles.cards}>
      <Card
        title="Connection"
        items={[
          ["Partner", settings.partner],
          ["Message", messageText(type)],
          ...destinationItems(connection, formatTime),
          ...workingItems(connection),
        ]}
      />
      <Card
        title="Checks and contract"
        items={[
          [
            "Destination check",
            verification
              ? `Verified by ${person(verification.verified.by, verification.verified.byDisplay)}, ${formatTime(verification.verified.at)} (${verification.method})`
              : "Not recorded: no patient data is written until it is",
          ],
          [
            "Last connection",
            lastAttempt
              ? `${formatTime(lastAttempt.at)} · ${lastAttempt.errorCode === null ? "OK" : problemText(lastAttempt.errorCode)}`
              : "None yet",
          ],
          // Only an MLLP partner answers; a folder or SFTP server never does.
          [
            "Awaiting application ACK",
            `${connection.awaitingAck} message${connection.awaitingAck === 1 ? "" : "s"}`,
          ],
          [
            "Last change",
            `${person(lastChange.by, lastChange.byDisplay)}, ${formatTime(lastChange.at)}${lastReason ? ` · reason: ${lastReason}` : ""}`,
          ],
        ]}
      />
      {(type === "hl7_oru" || type === "hl7_orders") && (
        <div className={styles.wide}>
          <CodeMappingsSection connection={connection} readOnly />
        </div>
      )}
    </div>
  );
}

/** The routing rules, one per line, and how they combine. */
export function ConnectionRules({ connection }: { connection: PartnerConnection }) {
  const { rules, intake } = connection.settings;
  if (intake) {
    return (
      <Card
        title="What to accept"
        items={[
          ["Accept messages from", `MSH-4 = ${intake.acceptFrom}`],
          ["Unknown patient", unknownPatientText(connection)],
          ["Unknown test code", refusedText(connection)],
          ["Replays", "An exact replay is ignored; the same ID with changed content is held"],
        ]}
      />
    );
  }
  return (
    <Card
      title="Routing rules"
      items={
        rules.length === 0
          ? [["Rules", noRulesText(connection.settings.messageType)]]
          : rules.map((rule, index) => [
              `Rule ${index + 1}`,
              ruleText(rule, connection.settings.messageType),
            ])
      }
    />
  );
}

/**
 * Code mappings, as in the mockup: an HL7 connection's, with their actions. Report PDF and
 * Billing JSON files carry the LIS's own codes, so they have no mapping to a partner's codes.
 */
export function CodeMappings({ connection }: { connection: PartnerConnection }) {
  if (["hl7_oru", "hl7_orders"].includes(connection.settings.messageType)) {
    return <CodeMappingsSection connection={connection} />;
  }
  return (
    <Section title="Code mappings">
      <p className={styles.hint}>
        No code mappings. Report PDF and Billing JSON files carry the LIS's own codes; mappings to a
        partner's test codes, LOINC and UCUM units apply to HL7 result connections.
      </p>
    </Section>
  );
}
