import type { AuConnectionSettings } from "@/generated/AuConnectionSettings";
import type { TransmissionClass } from "@/generated/TransmissionClass";
import { type Choice, checked, choiceOf, YES_NO } from "@/lib/choices";

export const AU_CLASSES: readonly Choice<TransmissionClass>[] = [
  { id: "a", label: "Class A — no acknowledgements" },
  { id: "b", label: "Class B — ACK / NAK" },
];
export const AU_BLOCK_LENGTHS = ["256", "512", "1024"].map((id) => ({ id, label: id }));
export const AU_RACK_DIGITS = [
  { id: "0", label: "Not sent" },
  { id: "4", label: "4 digits" },
  { id: "5", label: "5 digits" },
];
export const AU_RESULT_DIGITS = ["6", "9"].map((id) => ({ id, label: `${id} characters` }));

/** Form text follows the engine paths; all choices start unset. */
export interface AuValues {
  protocol: {
    framing: {
      start: string;
      end: string;
      bcc: string | null;
      use_etb: string | null;
      max_wire_len: string | null;
      class: string | null;
    };
    format: {
      unit_no: string | null;
      rack_digits: string | null;
      sample_id_digits: string;
      sex: string | null;
      age: string | null;
      month: string | null;
      patient_info_digits: [string, string, string, string, string, string];
      result_digits: string | null;
    };
  };
  frameTimeoutMs: string;
  blockTimeoutMs: string;
}

export const EMPTY_AU: AuValues = {
  protocol: {
    framing: { start: "", end: "", bcc: null, use_etb: null, max_wire_len: null, class: null },
    format: {
      unit_no: null,
      rack_digits: null,
      sample_id_digits: "",
      sex: null,
      age: null,
      month: null,
      patient_info_digits: ["", "", "", "", "", ""],
      result_digits: null,
    },
  },
  frameTimeoutMs: "",
  blockTimeoutMs: "",
};

const yesNo = (value: boolean) => (value ? "yes" : "no");
const booleanOf = (value: string | null) => checked(choiceOf(YES_NO, value)) === "yes";
const hex = (bytes: number[]) =>
  bytes.map((b) => b.toString(16).padStart(2, "0").toUpperCase()).join(" ");
const bytesOf = (value: string) =>
  value
    .trim()
    .split(/\s+/)
    .map((byte) => Number.parseInt(byte, 16));

export function auValues(settings: AuConnectionSettings): AuValues {
  const { framing: frame, format } = settings.protocol;
  const [a, b, c, d, e, f] = format.patient_info_digits;
  return {
    protocol: {
      framing: {
        start: hex(frame.start),
        end: hex(frame.end),
        bcc: yesNo(frame.bcc),
        use_etb: yesNo(frame.use_etb),
        max_wire_len: String(frame.max_wire_len),
        class: frame.class,
      },
      format: {
        unit_no: yesNo(format.unit_no),
        rack_digits: String(format.rack_digits),
        sample_id_digits: String(format.sample_id_digits),
        sex: yesNo(format.sex),
        age: yesNo(format.age),
        month: yesNo(format.month),
        result_digits: String(format.result_digits),
        patient_info_digits: [String(a), String(b), String(c), String(d), String(e), String(f)],
      },
    },
    frameTimeoutMs: String(settings.frameTimeoutMs),
    blockTimeoutMs: String(settings.blockTimeoutMs),
  };
}

/** Only protocol configuration is converted to numbers; result values never pass here. */
export function toAu(values: AuValues): AuConnectionSettings {
  const { framing: frame, format } = values.protocol;
  const [a, b, c, d, e, f] = format.patient_info_digits;
  const integer = (value: string) => Number.parseInt(value, 10);
  return {
    protocol: {
      framing: {
        start: bytesOf(frame.start),
        end: bytesOf(frame.end),
        bcc: booleanOf(frame.bcc),
        use_etb: booleanOf(frame.use_etb),
        max_wire_len: integer(checked(choiceOf(AU_BLOCK_LENGTHS, frame.max_wire_len))),
        class: checked(choiceOf(AU_CLASSES, frame.class)),
      },
      format: {
        unit_no: booleanOf(format.unit_no),
        rack_digits: integer(checked(choiceOf(AU_RACK_DIGITS, format.rack_digits))),
        sample_id_digits: integer(format.sample_id_digits),
        sex: booleanOf(format.sex),
        age: booleanOf(format.age),
        month: booleanOf(format.month),
        patient_info_digits: [
          integer(a),
          integer(b),
          integer(c),
          integer(d),
          integer(e),
          integer(f),
        ],
        result_digits: integer(checked(choiceOf(AU_RESULT_DIGITS, format.result_digits))),
      },
    },
    frameTimeoutMs: integer(values.frameTimeoutMs),
    blockTimeoutMs: integer(values.blockTimeoutMs),
  };
}
