// The Add/Edit connection form's values and the engine settings they make. The engine checks
// every value again; field errors under `settings.` land on the matching form field.
import type { PartnerConnection } from "@/generated/PartnerConnection";
import type { PartnerConnectionSettings } from "@/generated/PartnerConnectionSettings";
import type { PartnerMessageType } from "@/generated/PartnerMessageType";
import type { PartnerProtocol } from "@/generated/PartnerProtocol";
import type { RuleCondition } from "@/generated/RuleCondition";
import type { RuleField } from "@/generated/RuleField";
import type { Schedule } from "@/generated/Schedule";
import type { SftpAuth } from "@/generated/SftpAuth";
import type { SftpSettings } from "@/generated/SftpSettings";
import type { FieldPaths } from "@/lib/errors";
import { directionText, isIncoming } from "../labels";
import {
  type IntakeValues,
  type MovedToValues,
  toIntake,
  toIntakeValues,
  toMovedToValues,
} from "./intakeForm";

interface RuleValues {
  field: RuleField | "";
  condition: RuleCondition | "";
  value: string;
}

/** The SFTP server as entered; `password` is sent apart from the settings and never shown. */
export interface SftpValues {
  host: string;
  port: string;
  username: string;
  auth: SftpAuth;
  fingerprint: string;
  remoteFolder: string;
  password: string;
}

interface ConnectionFields {
  name: string;
  partner: string;
  messageType: string;
  /** Shown only: the message type fixes it. */
  direction: string;
  protocol: PartnerProtocol;
  /** Shown only: the message type fixes what makes a message. */
  sendWhen: string;
  /** The chosen folder, shown; the engine keeps it behind `folderChoice`. */
  folder: string;
  /** The handle of a newly chosen folder; empty keeps the current one. */
  folderChoice: string;
  sftp: SftpValues;
  /** For incoming orders: how the input folder is read and what is accepted. */
  intake: IntakeValues;
  rules: RuleValues[];
  schedule: { kind: string; minutes: string; time: string };
  fileNamePattern: string;
  reason: string;
}

/** The whole form: the connection's values and, for orders, its local Processed and Failed. */
export type ConnectionValues = ConnectionFields & MovedToValues;

/** Engine paths `settings.x` are form fields `x`; each folder handle shows on its folder. */
export const FIELD_PATHS: FieldPaths = {
  settings: "",
  folderChoice: "folder",
  processedFolderChoice: "processedFolder",
  failedFolderChoice: "failedFolder",
};

function toSftpValues(sftp: SftpSettings | null | undefined): SftpValues {
  return {
    host: sftp?.host ?? "",
    port: String(sftp?.port ?? 22),
    username: sftp?.username ?? "",
    auth: sftp?.auth ?? "key",
    fingerprint: sftp?.fingerprint ?? "",
    remoteFolder: sftp?.remoteFolder ?? "",
    password: "",
  };
}

export function toValues(connection: PartnerConnection | null): ConnectionValues {
  const settings = connection?.settings;
  const schedule = settings?.schedule;
  const protocol = settings?.protocol ?? "folder";
  return {
    name: settings?.name ?? "",
    partner: settings?.partner ?? "",
    messageType: settings?.messageType ?? "",
    direction: directionText(settings?.messageType ?? ""),
    protocol,
    // The message type fixes what makes a message; its option shares the type's key.
    sendWhen: settings?.messageType ?? "",
    folder: protocol === "folder" ? (connection?.folder ?? "") : "",
    folderChoice: "",
    ...toMovedToValues(connection),
    sftp: toSftpValues(settings?.sftp),
    intake: toIntakeValues(settings?.intake),
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

/** The server settings as entered; the engine trims and checks them. */
export function toSftp({ password: _, ...values }: SftpValues): SftpSettings {
  return { ...values, port: Number(values.port) };
}

/** The password to send: typed, for password sign-in; otherwise none (a saved one is kept). */
export function typedPassword(values: SftpValues): string | undefined {
  return values.auth === "password" && values.password !== "" ? values.password : undefined;
}

/** The settings to save. An incoming connection routes, schedules and names nothing. */
export function toSettings(values: ConnectionValues): PartnerConnectionSettings {
  const incoming = isIncoming(values.messageType);
  return {
    name: values.name.trim(),
    partner: values.partner.trim(),
    messageType: values.messageType as PartnerMessageType,
    protocol: values.protocol,
    rules: incoming
      ? []
      : values.rules.flatMap(({ field, condition, value }) =>
          field === "" || condition === "" ? [] : [{ field, condition, value: value.trim() }],
        ),
    schedule: incoming ? { kind: "immediately" } : toSchedule(values.schedule),
    fileNamePattern: incoming ? "" : values.fileNamePattern.trim(),
    sftp: values.protocol === "sftp" ? toSftp(values.sftp) : null,
    intake: incoming ? toIntake(values.intake, values.protocol) : null,
  };
}
