import { Radio, RadioGroup } from "react-aria-components";
import styles from "./ConnectionDialog.module.css";

/** The protocols of the Interfaces mockup; only Folder is available in this release. */
const PROTOCOLS = [
  { id: "folder", label: "Folder", hint: "Shared folder on the lab network", available: true },
  {
    id: "sftp",
    label: "SFTP",
    hint: "Partner's server, over the internet, encrypted",
    available: false,
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
 * Section 2, the protocol, as one choice among cards. Folder is the only protocol this release
 * writes with; the others are shown, not offered, and the line below says why.
 */
export function ProtocolCards() {
  return (
    <>
      <RadioGroup
        aria-label="Protocol"
        orientation="horizontal"
        value="folder"
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
      <p className={styles.hint}>SFTP and MLLP are not available yet.</p>
    </>
  );
}
