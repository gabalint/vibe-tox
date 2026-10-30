import { useFormContext, useWatch } from "react-hook-form";
import { FieldGrid, FieldSection } from "@/components/Form";
import { ChoiceInput, TextInput } from "@/components/FormFields";
import styles from "@/styles/layout.module.css";
import { WHOLE_NUMBER_FORMAT } from "../formats";
import { AuFields } from "./AuFields";
import {
  AU_DATA_BITS,
  CONNECTION_KINDS,
  type ConnectionValues,
  DATA_BITS,
  isAuKind,
  PARITIES,
  STOP_BITS,
} from "./connectionValues";

/**
 * The analyzer connection, inside a form whose values hold it as `connection`. An AU400/AU640
 * connection is long, so its fields sit side by side in titled sections of a wide dialog;
 * other connections keep one column.
 */
export function ConnectionFields() {
  const kind = useWatch<{ connection: ConnectionValues }, "connection.kind">({
    name: "connection.kind",
  });
  if (!isAuKind(kind)) {
    return (
      <>
        <ConnectionInputs kind={kind} />
        <ConnectionHint kind={kind} />
      </>
    );
  }
  return (
    <>
      <FieldSection title="Connection">
        <ConnectionHint kind={kind} />
        <FieldGrid>
          <ConnectionInputs kind={kind} />
        </FieldGrid>
      </FieldSection>
      <AuFields />
    </>
  );
}

function ConnectionInputs({ kind }: { kind: ConnectionValues["kind"] }) {
  const form = useFormContext<{ connection: ConnectionValues }>();
  const isAu = isAuKind(kind);
  return (
    <>
      <ChoiceInput
        name="connection.kind"
        label="Connection"
        options={CONNECTION_KINDS}
        isRequired
        onChange={(next) => {
          const bits = form.getValues("connection.dataBits");
          if (next === "au_serial" && bits !== "seven" && bits !== "eight") {
            form.setValue("connection.dataBits", null);
          }
        }}
      />
      {(kind === "astm_tcp" || kind === "au_tcp") && (
        <>
          <TextInput
            name="connection.bindAddress"
            label="Listen on address"
            description="This computer's address on the laboratory network, or 127.0.0.1."
            isRequired
          />
          <TextInput name="connection.port" label="Port" format={WHOLE_NUMBER_FORMAT} isRequired />
          <TextInput
            name="connection.allowedPeers"
            label="Analyzer addresses allowed to connect"
            description="Separate addresses with commas. Connections from any other address are refused."
            isRequired
          />
        </>
      )}
      {(kind === "astm_serial" || kind === "au_serial") && (
        <>
          <TextInput
            name="connection.portPath"
            label="Serial port"
            description="Such as COM3 or /dev/ttyUSB0."
            isRequired
          />
          <TextInput
            name="connection.baudRate"
            label="Baud rate"
            format={
              isAu
                ? {
                    pattern: /^(2400|4800|9600)$/,
                    message: "Enter 2400, 4800 or 9600.",
                    description: "AU400/AU640 supports 2400, 4800 or 9600 baud.",
                  }
                : WHOLE_NUMBER_FORMAT
            }
            isRequired
          />
          <ChoiceInput
            name="connection.dataBits"
            label="Data bits"
            options={isAu ? AU_DATA_BITS : DATA_BITS}
            isRequired
          />
          <ChoiceInput name="connection.parity" label="Parity" options={PARITIES} isRequired />
          <ChoiceInput
            name="connection.stopBits"
            label="Stop bits"
            options={STOP_BITS}
            isRequired
          />
        </>
      )}
    </>
  );
}

/** What the chosen kind of connection needs beyond its fields. */
function ConnectionHint({ kind }: { kind: ConnectionValues["kind"] }) {
  if (kind === "au_tcp") {
    return (
      <p className={styles.hint}>
        A transparent serial-to-TCP bridge connects to this LIS listener. Listen on this computer's
        address and add the bridge's address to the allowed list.
      </p>
    );
  }
  if (kind === "result_file") {
    return (
      <p className={styles.hint}>
        The instrument's software exports result files. After registering, choose the folder it
        exports to, or load each file by hand under Result imports.
      </p>
    );
  }
  return null;
}
