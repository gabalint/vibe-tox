import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { HostHealthView } from "@/generated/HostHealthView";
import layout from "@/styles/layout.module.css";
import { HealthBadge, keepAwakeState, NONE } from "./healthLabels";

/**
 * What the engine reports about this computer and whether it is kept awake (REQ-OPS-013),
 * in short columns. It sits above the checks, which take the remaining height.
 */
export function HostSection({ health }: { health: HostHealthView }) {
  const { host, keepAwake } = health;
  const awake = keepAwakeState(keepAwake.state);
  return (
    <Section title="This computer">
      <Facts
        layout="compact"
        items={[
          { label: "Overall", value: <HealthBadge state={health.overall} /> },
          { label: "Operating system", value: host.os },
          { label: "Version", value: host.osVersion },
          { label: "Edition", value: host.edition ?? NONE },
          { label: "Architecture", value: host.arch },
          { label: "Web engine", value: host.webview ?? NONE },
          { label: "App version", value: host.appVersion },
          {
            label: "Data folder",
            value: <span className={layout.code}>{host.dataFolder}</span>,
          },
          {
            label: "Keep awake",
            value: <StatusBadge tone={awake.tone}>{awake.label}</StatusBadge>,
          },
          {
            label: "Since",
            value: keepAwake.since ? <Timestamp value={keepAwake.since} /> : NONE,
          },
          { label: "Problem code", value: keepAwake.code ?? NONE },
        ]}
      />
    </Section>
  );
}
