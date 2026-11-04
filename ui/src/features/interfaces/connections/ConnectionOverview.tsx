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
import styles from "./ConnectionPage.module.css";
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

/**
 * Where it writes, as in the mockup: the protocol and folder, or for SFTP the protocol with
 * the server, the remote folder and the sign-in with the server's fingerprint.
 */
const code = (key: string, text: string) => (
  <span key={key} className={styles.code}>
    {text}
  </span>
);

function destinationItems({ folder, settings }: PartnerConnection): [string, ReactNode][] {
  const { sftp } = settings;
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
    return [
      ["Processed folder", code("processed", connection.processedFolder ?? "")],
      ["Failed folder", code("failed", connection.failedFolder ?? "")],
      ["Check every", `${intake.checkEverySeconds} seconds`],
      ["Accept messages from", `MSH-4 = ${intake.acceptFrom}`],
      ["Unknown patient", UNKNOWN_PATIENT_TEXT[intake.unknownPatient]],
    ];
  }
  const type = settings.messageType;
  return [
    ["Send when", `${SEND_WHEN[type]} · ${scheduleText(settings.schedule).toLowerCase()}`],
    ["File name", code("file-name", settings.fileNamePattern)],
    ["Rules", rulesText(connection)],
  ];
}

const UNKNOWN_PATIENT_TEXT = {
  hold: "Held until the patient is registered",
  reject: "Moved to Failed and logged",
} as const;

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
          ...destinationItems(connection),
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
          // A folder or SFTP server never answers, so nothing written there waits for an ACK.
          ["Awaiting application ACK", "0 messages"],
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
          ["Unknown patient", UNKNOWN_PATIENT_TEXT[intake.unknownPatient]],
          ["Unknown test code", "Moved to Failed and logged"],
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
