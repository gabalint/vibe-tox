import { Fragment, type ReactNode } from "react";
import { Section } from "@/components/Section";
import type { PartnerConnection } from "@/generated/PartnerConnection";
import { useFormatTime } from "@/lib/format";
import { usePersonText } from "@/lib/person";
import {
  MESSAGE_TYPES,
  PROTOCOLS,
  problemText,
  ruleText,
  SEND_WHEN,
  scheduleText,
} from "../labels";
import styles from "./ConnectionPage.module.css";

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

const rulesText = (connection: PartnerConnection) =>
  connection.settings.rules.length === 0
    ? "None: every message of this type"
    : connection.settings.rules.map(ruleText).join("; ");

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
          ["Message", `${MESSAGE_TYPES[type]} (bil-handoff-v2)`],
          ["Protocol", PROTOCOLS[settings.protocol]],
          [
            "Folder",
            <span key="folder" className={styles.code}>
              {connection.folder}
            </span>,
          ],
          ["Send when", `${SEND_WHEN[type]} · ${scheduleText(settings.schedule).toLowerCase()}`],
          [
            "File name",
            <span key="file-name" className={styles.code}>
              {settings.fileNamePattern}
            </span>,
          ],
          ["Rules", rulesText(connection)],
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
          // A folder never answers, so nothing written to one waits for an acknowledgement.
          ["Awaiting application ACK", "0 messages"],
          [
            "Last change",
            `${person(lastChange.by, lastChange.byDisplay)}, ${formatTime(lastChange.at)}${lastReason ? ` · reason: ${lastReason}` : ""}`,
          ],
        ]}
      />
    </div>
  );
}

/** The routing rules, one per line, and how they combine. */
export function ConnectionRules({ connection }: { connection: PartnerConnection }) {
  const { rules } = connection.settings;
  return (
    <Card
      title="Routing rules"
      items={
        rules.length === 0
          ? [["Rules", "None: every billing handoff goes to this connection"]]
          : rules.map((rule, index) => [`Rule ${index + 1}`, ruleText(rule)])
      }
    />
  );
}

/**
 * Code mappings, as in the mockup. A Billing JSON file carries the LIS's own codes, so it has
 * no mapping to a partner's codes; mappings come with HL7 result connections.
 */
export function CodeMappings() {
  return (
    <Section title="Code mappings">
      <p className={styles.hint}>
        No code mappings. A Billing JSON file carries the LIS's own codes; mappings to a partner's
        test codes, LOINC and UCUM units apply to HL7 result connections.
      </p>
    </Section>
  );
}
