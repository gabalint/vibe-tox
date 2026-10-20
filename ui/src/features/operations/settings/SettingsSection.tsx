import { useState } from "react";
import { Button } from "@/components/Button";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import type { CommandName } from "@/generated/CommandName";
import { useCommandQuery } from "@/lib/commands";
import { SettingsDialog, type SettingsDialogProps } from "./SettingsDialog";
import { SettingsFacts } from "./SettingsFacts";
import type { SettingsView } from "./settingFields";

interface SettingsSectionProps<Values extends Record<keyof Values, number>>
  extends Omit<SettingsDialogProps<Values>, "view" | "onClose"> {
  /** The section's heading, such as "Security policy"; its lower case names the loading states. */
  heading: string;
  /** The `get_…` query. */
  query: CommandName;
}

/** One versioned settings record: its values, and a dialog that sets the next version. */
export function SettingsSection<Values extends Record<keyof Values, number>>({
  heading,
  query,
  ...dialog
}: SettingsSectionProps<Values>) {
  const view = useCommandQuery<SettingsView<Values>>(query);
  const [isOpen, setOpen] = useState(false);
  return (
    <Section
      title={heading}
      actions={view.data && <Button onPress={() => setOpen(true)}>{dialog.title}</Button>}
    >
      <QueryContent query={view} what={heading.toLowerCase()}>
        {(data) => <SettingsFacts fields={dialog.fields} view={data} />}
      </QueryContent>
      {isOpen && view.data && (
        <SettingsDialog {...dialog} view={view.data} onClose={() => setOpen(false)} />
      )}
    </Section>
  );
}
