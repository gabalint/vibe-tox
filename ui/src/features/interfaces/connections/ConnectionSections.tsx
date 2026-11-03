import type { ReactNode } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { FieldSection } from "@/components/Form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import { MESSAGE_TYPES, SEND_WHEN } from "../labels";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

const TYPE_OPTIONS = Object.entries(MESSAGE_TYPES).map(([id, label]) => ({ id, label }));
const SEND_WHEN_OPTIONS = Object.entries(SEND_WHEN).map(([id, label]) => ({ id, label }));
const SCHEDULE_OPTIONS = [
  { id: "immediately", label: "Immediately" },
  { id: "every_minutes", label: "Every N minutes" },
  { id: "daily_at", label: "Daily at a time" },
];

/** One numbered section of the dialog in its own box. */
export function Box({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className={styles.box}>
      <FieldSection title={title}>{children}</FieldSection>
    </div>
  );
}

/** Section 1: name, partner, message type and the direction it fixes. */
export function GeneralSection({ isEdit }: { isEdit: boolean }) {
  return (
    <Box title="1. General">
      <div className={styles.row}>
        <TextInput name="name" label="Name" isRequired autoFocus />
        <TextInput name="partner" label="Partner (client)" isRequired />
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
  return (
    <Box title="5. When to send">
      <div className={styles.row}>
        <ChoiceInput name="sendWhen" label="Send when" options={SEND_WHEN_OPTIONS} isRequired />
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
          description="{handoff} is required; {date} and {time} are optional; ends in .json."
          isRequired
        />
      </div>
    </Box>
  );
}
