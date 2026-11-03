import { useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ReasonConfirmDialog } from "@/components/ReasonConfirmDialog";
import { Section } from "@/components/Section";
import { Select } from "@/components/Select";
import { useToast } from "@/components/Toast";
import type { ResolveUnroutedMessage } from "@/generated/ResolveUnroutedMessage";
import type { UnroutedDecision } from "@/generated/UnroutedDecision";
import type { UnroutedMessage } from "@/generated/UnroutedMessage";
import { useCommandMutation } from "@/lib/commands";
import { useFormatTime } from "@/lib/format";
import row from "../actionRow.module.css";
import { MESSAGE_QUERIES, RESOLVE_UNROUTED } from "../contract";
import { MESSAGE_TYPES, recordText, ruleText, valuesText } from "../labels";
import { CONNECTIONS_PATH, connectionPath } from "../paths";
import { AnotherWayDialog } from "./AnotherWayDialog";
import styles from "./UnroutedPage.module.css";

/**
 * The chosen unrouted message, below the table as in the mockup: when it was made, which
 * connections it was checked against, the closest rule and why it did not match, and the
 * decisions. Send once and Retry routing make deliveries the usual checks still apply to;
 * Add a rule opens the closest connection's rules.
 */
export function UnroutedDetails({ message }: { message: UnroutedMessage }) {
  const navigate = useNavigate();
  const toast = useToast();
  const formatTime = useFormatTime();
  const { closest, facts, connections } = message;
  const [target, setTarget] = useState<string | null>(
    closest?.connection.id ?? connections[0]?.id ?? null,
  );
  const [open, setOpen] = useState<"another" | "dismiss" | null>(null);
  const resolve = useCommandMutation<unknown, { input: ResolveUnroutedMessage }>(RESOLVE_UNROUTED, {
    invalidates: MESSAGE_QUERIES,
  });
  const decide = (decision: UnroutedDecision, title: string) =>
    resolve.mutate(
      { input: { messageId: message.messageId, decision } },
      {
        onSuccess: () => {
          toast.show({ tone: "success", title });
          setOpen(null);
        },
      },
    );
  const type = MESSAGE_TYPES[message.messageType];
  const client = facts.clients[0]?.label;
  const allowed = message.allowedActions.includes(RESOLVE_UNROUTED);
  const fact = (label: string, value: string) => (
    <div className={styles.fact}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
  return (
    <Section title={`${recordText(message)} – ${type} message`}>
      <dl className={styles.facts}>
        {fact("Created", `${formatTime(message.createdAt)}, at handoff creation`)}
        {fact(
          "Checked against",
          `${message.checkedAgainst} ${type} connection${message.checkedAgainst === 1 ? "" : "s"}`,
        )}
        {fact("Client", valuesText(facts.clients))}
        {fact("Closest rule", closest ? `${ruleText(closest.rule)} (did not match)` : "None")}
      </dl>
      {allowed && (
        <div className={row.row}>
          <div className={styles.target}>
            <Select
              label="Send to"
              isLabelHidden
              name="target"
              placeholder="Send to…"
              options={connections.map(({ id, name }) => ({ id, label: `Send to: ${name}` }))}
              value={target}
              onChange={setTarget}
            />
          </div>
          <Button
            variant="primary"
            isDisabled={target === null}
            isPending={resolve.isPending}
            onPress={() =>
              target && decide({ action: "send_once", connection: target }, "Sent once")
            }
          >
            Send once
          </Button>
          <Button
            onPress={() =>
              navigate(
                closest ? `${connectionPath(closest.connection.id)}/rules` : CONNECTIONS_PATH,
              )
            }
          >
            {client ? `Add a rule for ${client}` : "Add a rule"}
          </Button>
          <Button onPress={() => decide({ action: "retry_routing" }, "Routed by the rules")}>
            Retry routing
          </Button>
          <Button onPress={() => setOpen("another")}>Deliver another way…</Button>
          <Button onPress={() => setOpen("dismiss")}>Dismiss with reason</Button>
        </div>
      )}
      <p className={styles.hint}>
        "Deliver another way" records a manual delivery (e.g. fax) so the file is still reconciled.
        Every action is audited.
      </p>
      {resolve.error && open === null && <ErrorState title="Not recorded" error={resolve.error} />}
      {open === "another" && (
        <AnotherWayDialog
          onClose={() => setOpen(null)}
          onSubmit={(method) =>
            decide({ action: "deliver_another_way", method }, "Delivery recorded")
          }
          command={resolve}
        />
      )}
      {open === "dismiss" && (
        <ReasonConfirmDialog
          title="Dismiss message"
          confirmLabel="Dismiss"
          variant="danger"
          command={resolve}
          input={(reason): ResolveUnroutedMessage => ({
            messageId: message.messageId,
            decision: { action: "dismiss", reason },
          })}
          successTitle="Message dismissed"
          onClose={() => setOpen(null)}
        >
          The message is not delivered. It stays in Logs as dismissed, with your reason.
        </ReasonConfirmDialog>
      )}
    </Section>
  );
}
