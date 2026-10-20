import type { OperationsSettings } from "@/generated/OperationsSettings";
import type { OperationsSettingsView } from "@/generated/OperationsSettingsView";
import type { SecurityPolicy } from "@/generated/SecurityPolicy";

/**
 * A versioned settings record around its values: `OperationsSettingsView` and
 * `SecurityPolicyView` share this shape. `version` is `null` while the defaults apply.
 */
export type SettingsView<Values> = Omit<OperationsSettingsView, "values" | "defaults"> & {
  values: Values;
  defaults: Values;
};

/**
 * One whole-number setting: its label, its unit in words and, where the design brief documents
 * one (§2.2, §8), the range the engine accepts. The engine checks the range; the screen only
 * shows it.
 */
export interface SettingField<Key extends string> {
  key: Key;
  label: string;
  unit: string;
  range?: string;
}

/** Operations settings (design brief §2.2, §3.4, §6.1); laboratory questions L2 to L6. */
export const OPERATIONS_FIELDS: readonly SettingField<keyof OperationsSettings>[] = [
  { key: "backupIntervalMinutes", label: "Backup interval", unit: "minutes", range: "5 to 1440" },
  { key: "keepRecent", label: "Recent backup sets kept", unit: "sets", range: "2 to 96" },
  { key: "keepDaily", label: "Daily backup sets kept", unit: "days", range: "0 to 90" },
  { key: "keepWeekly", label: "Weekly backup sets kept", unit: "weeks", range: "0 to 104" },
  { key: "backupWarnMinutes", label: "Backup age warning", unit: "minutes" },
  { key: "backupAlertMinutes", label: "Backup age alert", unit: "minutes" },
  { key: "offhostWarnHours", label: "Off-host copy age warning", unit: "hours" },
  { key: "offlineWarnDays", label: "Offline copy age warning", unit: "days" },
  { key: "restoreTestWarnDays", label: "Restore test age warning", unit: "days" },
  { key: "kitConfirmWarnDays", label: "Recovery kit confirmation warning", unit: "days" },
  { key: "anchorWarnMinutes", label: "Audit anchor lag warning", unit: "minutes" },
  { key: "anchorIncidentHours", label: "Audit anchor lag incident", unit: "hours" },
  { key: "diskFreeWarnGib", label: "Free disk space warning", unit: "GiB" },
  { key: "osSupportWarnDays", label: "Warning before operating system support ends", unit: "days" },
  { key: "clockJumpSeconds", label: "Clock jump recorded as a gap above", unit: "seconds" },
];

export type NumericSecurityPolicy = Pick<
  SecurityPolicy,
  "idleTimeoutMinutes" | "lockoutFailures" | "lockoutMinutes"
>;

/** The numeric security policy (design brief §8); laboratory question L11. */
export const SECURITY_FIELDS: readonly SettingField<keyof NumericSecurityPolicy>[] = [
  { key: "idleTimeoutMinutes", label: "Idle timeout", unit: "minutes", range: "2 to 60" },
  {
    key: "lockoutFailures",
    label: "Failed sign-ins before lockout",
    unit: "attempts",
    range: "3 to 10",
  },
  { key: "lockoutMinutes", label: "Lockout duration", unit: "minutes", range: "5 to 60" },
];
