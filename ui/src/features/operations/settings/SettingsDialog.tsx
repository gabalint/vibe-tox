import type { FieldValues } from "react-hook-form";
import { type TextFormat, TextInput } from "@/components/FormFields";
import { ReasonField } from "@/components/ReasonField";
import type { CommandName } from "@/generated/CommandName";
import { CommandDialog } from "@/lib/CommandDialog";
import type { SettingField, SettingsView } from "./settingFields";

const WHOLE_NUMBER: TextFormat = {
  pattern: /^\d+$/,
  message: "Enter a whole number.",
  description: "A whole number",
};

export interface SettingsDialogProps<Values extends Record<keyof Values, number>> {
  title: string;
  /** The `set_…` command and the queries it makes stale. */
  command: { name: CommandName; invalidates: readonly CommandName[] };
  fields: readonly SettingField<keyof Values & string>[];
  /** Whether the command takes the values under `values` or at the root of its input. */
  nested: boolean;
  view: SettingsView<Values>;
  successTitle: string;
  onClose: () => void;
}

function description({ unit, range }: SettingField<string>, defaultValue: number): string {
  const allowed = range ? ` The app accepts ${range}.` : "";
  return `In ${unit}. Default ${defaultValue}.${allowed}`;
}

export function SettingsFields<Values extends Record<keyof Values, number>>({
  fields,
  view,
  nested,
}: Pick<SettingsDialogProps<Values>, "fields" | "view" | "nested">) {
  return fields.map((field) => (
    <TextInput
      key={field.key}
      name={`${nested ? "values." : ""}${field.key}`}
      label={field.label}
      format={{ ...WHOLE_NUMBER, description: description(field, view.defaults[field.key]) }}
      isRequired
    />
  ));
}

/**
 * Sets a new version of a settings record, prefilled from the current one. The screen checks
 * only that each value is a whole number; the engine checks the ranges. Every version after the
 * first needs a reason. Field names match the engine's paths (`values.keepRecent`, or
 * `idleTimeoutMinutes`), so its field errors land on the matching fields.
 */
export function SettingsDialog<Values extends Record<keyof Values, number>>({
  fields,
  nested,
  view,
  ...props
}: SettingsDialogProps<Values>) {
  const texts = Object.fromEntries(fields.map(({ key }) => [key, String(view.values[key])]));
  const defaultValues = nested ? { values: texts, reason: "" } : { ...texts, reason: "" };
  const toInput = (form: FieldValues) => {
    const typed: Record<string, string> = nested ? form.values : form;
    const values = Object.fromEntries(fields.map(({ key }) => [key, Number(typed[key]?.trim())]));
    return {
      expectedVersion: view.version,
      ...(nested ? { values } : values),
      reason: view.version === null ? null : String(form.reason).trim(),
    };
  };
  return (
    <CommandDialog<FieldValues, Record<string, unknown>>
      {...props}
      submitLabel="Save settings"
      defaultValues={defaultValues}
      toInput={toInput}
    >
      <SettingsFields fields={fields} view={view} nested={nested} />
      {view.version !== null && <ReasonField />}
    </CommandDialog>
  );
}
