import type { AuConnectionSettings } from "@/generated/AuConnectionSettings";
import type { InstrumentConnectionSettings } from "@/generated/InstrumentConnectionSettings";
import { labelOf } from "@/lib/choices";
import { AU_CLASSES, AU_RACK_DIGITS, AU_RESULT_DIGITS, auValues } from "./auValues";
import { DATA_BITS, PARITIES, STOP_BITS } from "./connectionValues";

interface Fact {
  label: string;
  value: string;
}

/**
 * A network or serial connection with the Edit connection form's labels, in its order. A
 * result-file instrument has nothing to connect to; its watched folder is shown beside these.
 */
export function connectionFacts(settings: InstrumentConnectionSettings): Fact[] {
  switch (settings.kind) {
    case "astm_tcp":
    case "au_tcp":
      return [
        { label: "Listen on address", value: settings.bindAddress },
        { label: "Port", value: String(settings.port) },
        { label: "Analyzer addresses allowed to connect", value: settings.allowedPeers.join(", ") },
      ];
    case "astm_serial":
    case "au_serial":
      return [
        { label: "Serial port", value: settings.portPath },
        { label: "Baud rate", value: String(settings.baudRate) },
        { label: "Data bits", value: labelOf(DATA_BITS, settings.dataBits) },
        { label: "Parity", value: labelOf(PARITIES, settings.parity) },
        { label: "Stop bits", value: labelOf(STOP_BITS, settings.stopBits) },
      ];
    case "result_file":
      return [];
  }
}

/** The AU parameter-sheet settings with the form's labels and wording, in its order. */
export function auFacts(settings: AuConnectionSettings): Fact[] {
  const { framing, format } = settings.protocol;
  // The start and end codes in the hexadecimal the form takes.
  const codes = auValues(settings).protocol.framing;
  const yesNo = (value: boolean) => (value ? "Yes" : "No");
  return [
    { label: "Transmission class", value: labelOf(AU_CLASSES, framing.class) },
    { label: "Start code (hex)", value: codes.start },
    { label: "End code (hex)", value: codes.end },
    { label: "BCC checksum used", value: yesNo(framing.bcc) },
    { label: "ETB block endings used", value: yesNo(framing.use_etb) },
    { label: "Maximum block length", value: `${framing.max_wire_len} bytes` },
    { label: "Frame receive timeout (ms)", value: String(settings.frameTimeoutMs) },
    { label: "Next block timeout (ms)", value: String(settings.blockTimeoutMs) },
    { label: "Instrument unit number sent", value: yesNo(format.unit_no) },
    {
      label: "Rack number width",
      value: labelOf(AU_RACK_DIGITS, String(format.rack_digits)),
    },
    { label: "Sample ID width", value: String(format.sample_id_digits) },
    {
      label: "Result field width",
      value: labelOf(AU_RESULT_DIGITS, String(format.result_digits)),
    },
    { label: "Sex field sent", value: yesNo(format.sex) },
    { label: "Age field sent", value: yesNo(format.age) },
    { label: "Month field sent", value: yesNo(format.month) },
    { label: "Patient information 1–6 widths", value: format.patient_info_digits.join(", ") },
  ];
}
