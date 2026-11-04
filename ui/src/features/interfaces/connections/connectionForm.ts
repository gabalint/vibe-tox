// The Add/Edit connection form's values and the engine settings they make. The engine checks
// every value again; field errors under `settings.` land on the matching form field.
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionSettings } from "@/generated/PartnerConnectionSettings";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import type { RuleCondition } from "@/generated/RuleCondition";
import type { RuleField } from "@/generated/RuleField";
import type { Schedule } from "@/generated/Schedule";
import type { FieldPaths } from "@/lib/errors";

interface RuleValues {
  field: RuleField | "";
  condition: RuleCondition | "";
  value: string;
}

export interface ConnectionValues {
  name: string;
  partner: string;
  messageType: string;
  /** Shown only: the message type fixes it. */
  direction: string;
  /** Shown only: the message type fixes what makes a message. */
  sendWhen: string;
  /** The chosen folder, shown; the engine keeps it behind `folderChoice`. */
  folder: string;
  /** The handle of a newly chosen folder; empty keeps the current one. */
  folderChoice: string;
  rules: RuleValues[];
  schedule: { kind: string; minutes: string; time: string };
  fileNamePattern: string;
  reason: string;
}

/** Engine paths `settings.x` are form fields `x`; the folder handle shows on the folder. */
export const FIELD_PATHS: FieldPaths = { settings: "", folderChoice: "folder" };

export function toValues(connection: PartnerConnection | null): ConnectionValues {
  const settings = connection?.settings;
  const schedule = settings?.schedule;
  return {
    name: settings?.name ?? "",
    partner: settings?.partner ?? "",
    messageType: settings?.messageType ?? "",
    direction: "Out – LIS sends",
    // The message type fixes what makes a message; its option shares the type's key.
    sendWhen: settings?.messageType ?? "",
    folder: connection?.folder ?? "",
    folderChoice: "",
    rules: settings?.rules.map((rule) => ({ ...rule })) ?? [],
    schedule: {
      kind: schedule?.kind ?? "immediately",
      minutes: schedule?.kind === "every_minutes" ? String(schedule.minutes) : "",
      time: schedule?.kind === "daily_at" ? schedule.time : "",
    },
    fileNamePattern: settings?.fileNamePattern ?? "",
    reason: "",
  };
}

function toSchedule(values: ConnectionValues["schedule"]): Schedule {
  switch (values.kind) {
    case "every_minutes":
      return { kind: "every_minutes", minutes: Number(values.minutes) };
    case "daily_at":
      return { kind: "daily_at", time: values.time.trim() };
    default:
      return { kind: "immediately" };
  }
}

export function toSettings(values: ConnectionValues): PartnerConnectionSettings {
  return {
    name: values.name.trim(),
    partner: values.partner.trim(),
    messageType: values.messageType as PartnerMessageType,
    protocol: "folder",
    rules: values.rules.flatMap(({ field, condition, value }) =>
      field === "" || condition === "" ? [] : [{ field, condition, value: value.trim() }],
    ),
    schedule: toSchedule(values.schedule),
    fileNamePattern: values.fileNamePattern.trim(),
  };
}
