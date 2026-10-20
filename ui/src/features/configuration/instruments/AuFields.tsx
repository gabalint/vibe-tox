import { ChoiceInput, type TextFormat, TextInput } from "@/components/FormFields";
import { YES_NO } from "@/lib/choices";
import styles from "@/styles/layout.module.css";
import fields from "./AuFields.module.css";
import { AU_BLOCK_LENGTHS, AU_CLASSES, AU_RACK_DIGITS, AU_RESULT_DIGITS } from "./auValues";

const HEX_CODES: TextFormat = {
  pattern: /^(?!06|15)(?:0[1-9a-f]|1[0-9a-f])(?:\s+(?!06|15)(?:0[1-9a-f]|1[0-9a-f]))?$/i,
  message: "Enter one or two hexadecimal bytes from 01 to 1F, excluding 06 and 15.",
  description:
    "Hexadecimal, such as 02 or 02 01. One byte means no second code. Use the analyzer's setting.",
};
const WIDTH: TextFormat = {
  pattern: /^(?:[0-9]|1[0-9]|20)$/,
  message: "Enter a whole number from 0 to 20.",
  description: "Number of characters; enter 0 when this field is not sent.",
};
const ID_WIDTH: TextFormat = {
  pattern: /^(?:[1-9]|1[0-9]|20)$/,
  message: "Enter a whole number from 1 to 20.",
  description: "Number of characters in the specimen barcode field, 1–20.",
};
const TIMER: TextFormat = {
  pattern: /^[1-9][0-9]?00$/,
  message: "Enter 100–9900 milliseconds in steps of 100.",
  description: "100–9900 milliseconds, in steps of 100. Match the analyzer's timing settings.",
};

/** Explicit AU400/AU640 parameter-sheet settings, grouped for review. */
export function AuFields() {
  return (
    <>
      <p className={styles.hint}>
        Copy the AU400/AU640 online parameter sheet exactly. New connections start empty. These
        settings describe the message format; they do not convert clinical results. Result reception
        is available. Automatic test requisition (TRI) lookup is unavailable.
      </p>
      <fieldset className={fields.group}>
        <legend className={fields.legend}>AU framing and timing</legend>
        <ChoiceInput
          name="connection.au.protocol.framing.class"
          label="Transmission class"
          options={AU_CLASSES}
          isRequired
        />
        <TextInput
          name="connection.au.protocol.framing.start"
          label="Start code (hex)"
          format={HEX_CODES}
          isRequired
        />
        <TextInput
          name="connection.au.protocol.framing.end"
          label="End code (hex)"
          format={HEX_CODES}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.framing.bcc"
          label="BCC checksum used"
          options={YES_NO}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.framing.use_etb"
          label="ETB block endings used"
          options={YES_NO}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.framing.max_wire_len"
          label="Maximum block length"
          options={AU_BLOCK_LENGTHS}
          description="Bytes, including the start code, end code and checksum."
          isRequired
        />
        <TextInput
          name="connection.au.frameTimeoutMs"
          label="Frame receive timeout (ms)"
          format={TIMER}
          isRequired
        />
        <TextInput
          name="connection.au.blockTimeoutMs"
          label="Next block timeout (ms)"
          format={TIMER}
          isRequired
        />
      </fieldset>
      <fieldset className={fields.group}>
        <legend className={fields.legend}>AU sample and result fields</legend>
        <ChoiceInput
          name="connection.au.protocol.format.unit_no"
          label="Instrument unit number sent"
          options={YES_NO}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.format.rack_digits"
          label="Rack number width"
          options={AU_RACK_DIGITS}
          description="Cup position is present when a rack number is sent."
          isRequired
        />
        <TextInput
          name="connection.au.protocol.format.sample_id_digits"
          label="Sample ID width"
          format={ID_WIDTH}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.format.result_digits"
          label="Result field width"
          options={AU_RESULT_DIGITS}
          description="Field length, not decimal precision. The two flag characters are separate."
          isRequired
        />
      </fieldset>
      <fieldset className={fields.group}>
        <legend className={fields.legend}>AU optional patient fields</legend>
        <ChoiceInput
          name="connection.au.protocol.format.sex"
          label="Sex field sent"
          options={YES_NO}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.format.age"
          label="Age field sent"
          options={YES_NO}
          isRequired
        />
        <ChoiceInput
          name="connection.au.protocol.format.month"
          label="Month field sent"
          options={YES_NO}
          isRequired
        />
        {[0, 1, 2, 3, 4, 5].map((index) => (
          <TextInput
            key={index}
            name={`connection.au.protocol.format.patient_info_digits.${index}`}
            label={`Patient information ${index + 1} width`}
            format={WIDTH}
            isRequired
          />
        ))}
      </fieldset>
    </>
  );
}
