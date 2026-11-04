import { Radio, RadioGroup } from "react-aria-components";
import { Controller, useFormContext } from "react-hook-form";
import styles from "./ConnectionDialog.module.css";
import type { ConnectionValues } from "./connectionForm";

/** The protocols of the Interfaces mockup; Folder and SFTP are available in this release. */
const PROTOCOLS = [
  { id: "folder", label: "Folder", hint: "Shared folder on the lab network", available: true },
  {
    id: "sftp",
    label: "SFTP",
    hint: "Partner's server, over the internet, encrypted",
    available: true,
  },
  {
    id: "mllp_connects",
    label: "MLLP – LIS connects",
    hint: "Real-time HL7 to the partner",
    available: false,
  },
  {
    id: "mllp_listens",
    label: "MLLP – LIS listens",
    hint: "Partner connects on the lab network only",
    available: false,
  },
] as const;

/**
 * Section 2, the protocol, as one choice among cards. Folder and SFTP write; MLLP is shown, not
 * offered, and the line below says why.
 */
export function ProtocolCards() {
  const { control } = useFormContext<ConnectionValues>();
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
                isDisabled={!protocol.available}
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
      <p className={styles.hint}>MLLP is not available yet.</p>
    </>
  );
}
