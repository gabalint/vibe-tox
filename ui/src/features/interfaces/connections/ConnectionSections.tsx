import { type ReactNode, useEffect, useRef } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { FieldSection } from "@/components/Form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import type { ClientView } from "@/generated/ClientView";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import { useCommandQuery } from "@/lib/commands";
import { LIST_CLIENTS } from "../contract";
import { directionText, FILE_NAMES, isIncoming, MESSAGE_OPTIONS, SEND_WHEN } from "../labels";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

const TYPE_OPTIONS = Object.entries(MESSAGE_OPTIONS).map(([id, label]) => ({ id, label }));
const SCHEDULE_OPTIONS = [
  { id: "immediately", label: "Immediately" },
  { id: "every_minutes", label: "Every N minutes" },
  { id: "daily_at", label: "Daily at a time" },
];

const fileNames = (type: string) =>
  FILE_NAMES[type as PartnerMessageType] as (typeof FILE_NAMES)[PartnerMessageType] | undefined;

/** One numbered section of the dialog in its own box. */
export function Box({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className={styles.box}>
      <FieldSection title={title}>{children}</FieldSection>
    </div>
  );
}

/**
 * The message type fixes "Send when"; choosing another type also swaps the file name for the
 * new type's default, unless someone already changed it.
 */
function useTypeDefaults() {
  const { control, getValues, setValue } = useFormContext<ConnectionValues>();
  const messageType = useWatch({ control, name: "messageType" });
  const previous = useRef(messageType);
  useEffect(() => {
    if (previous.current === messageType) return;
    const pattern = getValues("fileNamePattern");
    if (pattern === "" || pattern === fileNames(previous.current)?.pattern) {
      setValue("fileNamePattern", fileNames(messageType)?.pattern ?? "");
    }
    setValue("sendWhen", messageType);
    setValue("direction", directionText(messageType));
    previous.current = messageType;
  }, [messageType, getValues, setValue]);
}

/**
 * The partner: free text for a connection that sends; for incoming orders, the laboratory's
 * client the orders are for, by code.
 */
function PartnerInput() {
  const { control } = useFormContext<ConnectionValues>();
  const messageType = useWatch({ control, name: "messageType" });
  const clients = useCommandQuery<ClientView[]>(LIST_CLIENTS, undefined, {
    enabled: isIncoming(messageType),
  });
  if (!isIncoming(messageType)) {
    return <TextInput name="partner" label="Partner (client)" isRequired />;
  }
  const options = (clients.data ?? [])
    .filter((client) => client.active)
    .map((client) => ({ id: client.code, label: `${client.code} – ${client.name}` }));
  return <ChoiceInput name="partner" label="Partner (client)" options={options} isRequired />;
}

/** Section 1: name, partner, message type and the direction it fixes. */
export function GeneralSection({ isEdit }: { isEdit: boolean }) {
  useTypeDefaults();
  return (
    <Box title="1. General">
      <div className={styles.row}>
        <TextInput name="name" label="Name" isRequired autoFocus />
        <PartnerInput />
        <ChoiceInput
          name="messageType"
          label="Message"
          options={TYPE_OPTIONS}
          isRequired
          isDisabled={isEdit}
        />
        <TextInput name="direction" label="Direction" isDisabled />
      </div>
    </Box>
  );
}

/** Section 5: what makes a message, when it is written, and the file's name. */
export function WhenSection() {
  const { control } = useFormContext<ConnectionValues>();
  const scheduleKind = useWatch({ control, name: "schedule.kind" });
  const messageType = useWatch({ control, name: "messageType" });
  const sendWhen =
    messageType === ""
      ? []
      : [{ id: messageType, label: SEND_WHEN[messageType as PartnerMessageType] }];
  return (
    <Box title="5. When to send">
      <div className={styles.row}>
        <ChoiceInput name="sendWhen" label="Send when" options={sendWhen} isRequired />
        <ChoiceInput
          name="schedule.kind"
          label="Schedule"
          options={SCHEDULE_OPTIONS}
          description="Or: every N minutes / daily at a time (e.g. billing)."
          isRequired
        />
        {scheduleKind === "every_minutes" && (
          <TextInput name="schedule.minutes" label="Minutes" description="1 to 1440" isRequired />
        )}
        {scheduleKind === "daily_at" && (
          <TextInput
            name="schedule.time"
            label="Time"
            description="HH:MM, laboratory time"
            isRequired
          />
        )}
        <TextInput
          name="fileNamePattern"
          label="File name"
          description={fileNames(messageType)?.hint ?? "Choose the message first."}
          isRequired
        />
      </div>
      {(messageType === "report_pdf" || messageType === "hl7_oru") && (
        <p className={styles.hint}>
          Corrections and withdrawals go first, to every connection that received the earlier
          revision{messageType === "hl7_oru" ? ", with HL7 corrected status" : ""}.
        </p>
      )}
    </Box>
  );
}
