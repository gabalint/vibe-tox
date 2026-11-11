import { Radio, RadioGroup } from "react-aria-components";
import { Controller, useFormContext, useWatch } from "react-hook-form";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

/**
 * The protocols of the Interfaces mockup and the message types each carries: a folder and SFTP
 * carry every type; the LIS connects over MLLP to send HL7 results, and listens over MLLP to
 * receive HL7 orders.
 */
const PROTOCOLS = [
  { id: "folder", label: "Folder", hint: "Shared folder on the lab network", carries: () => true },
  {
    id: "sftp",
    label: "SFTP",
    hint: "Partner's server, over the internet, encrypted",
    carries: () => true,
  },
  {
    id: "mllp_connect",
    label: "MLLP – LIS connects",
    hint: "Real-time HL7 to the partner",
    carries: (type: string) => type === "hl7_oru",
  },
  {
    id: "mllp_listen",
    label: "MLLP – LIS listens",
    hint: "Partner connects on the lab network only",
    carries: (type: string) => type === "hl7_orders",
  },
] as const;

/**
 * Section 2, the protocol, as one choice among cards. A card the chosen message cannot travel
 * by is shown, not offered, and the line below says which messages MLLP carries.
 */
export function ProtocolCards() {
  const { control } = useFormContext<ConnectionValues>();
  const messageType = useWatch({ control, name: "messageType" });
  return (
    <>
      <Controller
        control={control}
        name="protocol"
        render={({ field }) => (
          <RadioGroup
            aria-label="Protocol"
            orientation="horizontal"
            value={field.value}
            onChange={(value) => field.onChange(value)}
            className={styles.cards}
          >
            {PROTOCOLS.map((protocol) => (
              <Radio
                key={protocol.id}
                value={protocol.id}
                isDisabled={!protocol.carries(messageType)}
                className={styles.card}
              >
                {({ isSelected }) => (
                  <>
                    <span className={styles.cardTitle}>
                      <span aria-hidden="true">{isSelected ? "◉" : "○"}</span> {protocol.label}
                    </span>
                    <span className={styles.cardHint}>{protocol.hint}</span>
                  </>
                )}
              </Radio>
            ))}
          </RadioGroup>
        )}
      />
      <p className={styles.hint}>
        MLLP – LIS connects sends HL7 Results. MLLP – LIS listens receives HL7 Orders.
      </p>
    </>
  );
}
