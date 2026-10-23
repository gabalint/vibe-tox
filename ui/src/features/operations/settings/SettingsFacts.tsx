import { Alert } from "@/components/Alert";
import { Facts } from "@/components/Facts";
import { Timestamp } from "@/components/Timestamp";
import { Person } from "../Person";
import type { SettingField, SettingsView } from "./settingFields";

interface SettingsFactsProps<Values extends Record<keyof Values, number>> {
  fields: readonly SettingField<keyof Values & string>[];
  view: SettingsView<Values>;
}

/**
 * Every value with its unit, marked "(default)" where it is the default, and who set the current
 * version. These are laboratory decisions: until the laboratory sets them, the defaults apply.
 */
export function SettingsFacts<Values extends Record<keyof Values, number>>({
  fields,
  view,
}: SettingsFactsProps<Values>) {
  const items = fields.map(({ key, label, unit }) => {
    const value = view.values[key];
    const marker = value === view.defaults[key] ? " (default)" : "";
    return { label, value: `${value} ${unit}${marker}` };
  });
  if (view.version === null) {
    return (
      <>
        <Alert tone="info" title="Defaults apply">
          The laboratory has not set these values yet. They are laboratory decisions; until they are
          made, the app uses the defaults shown.
        </Alert>
        <Facts items={items} />
      </>
    );
  }
  const setAt = view.setAt ? <Timestamp value={view.setAt} /> : "Unknown";
  return (
    <Facts
      items={[
        ...items,
        { label: "Version", value: String(view.version) },
        {
          label: "Set",
          value: (
            <>
              {setAt} by{" "}
              {view.setBy ? <Person id={view.setBy} display={view.setByDisplay} /> : "Unknown"}
            </>
          ),
        },
      ]}
    />
  );
}
