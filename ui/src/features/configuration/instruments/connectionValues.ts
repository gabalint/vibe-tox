import type { InstrumentConnectionSettings } from "@/generated/InstrumentConnectionSettings";
import type { SerialDataBits } from "@/generated/SerialDataBits";
import type { SerialParity } from "@/generated/SerialParity";
import type { SerialStopBits } from "@/generated/SerialStopBits";
import { type Choice, checked, choiceOf, labelOf } from "@/lib/choices";
import { type AuValues, auValues, EMPTY_AU, toAu } from "./auValues";

type ConnectionKind = InstrumentConnectionSettings["kind"];

export const CONNECTION_KINDS: readonly Choice<ConnectionKind>[] = [
  { id: "astm_tcp", label: "Network (ASTM over TCP/IP)" },
  { id: "astm_serial", label: "Serial port (ASTM)" },
  { id: "au_tcp", label: "AU400 / AU640 — network bridge" },
  { id: "au_serial", label: "AU400 / AU640 — serial port" },
  { id: "result_file", label: "Result files" },
];

export const DATA_BITS: readonly Choice<SerialDataBits>[] = [
  { id: "five", label: "5" },
  { id: "six", label: "6" },
  { id: "seven", label: "7" },
  { id: "eight", label: "8" },
];

export const AU_DATA_BITS = DATA_BITS.filter(({ id }) => id === "seven" || id === "eight");

export const PARITIES: readonly Choice<SerialParity>[] = [
  { id: "none", label: "None" },
  { id: "odd", label: "Odd" },
  { id: "even", label: "Even" },
];

export const STOP_BITS: readonly Choice<SerialStopBits>[] = [
  { id: "one", label: "1" },
  { id: "two", label: "2" },
];

/**
 * What an instrument's `enabled` switches on, in words: listening for an analyzer, or picking
 * up files from a result-file instrument's watched folder (F13).
 */
export function pickupName(kind: ConnectionKind | null): string {
  return kind === "result_file" ? "folder pickup" : "listening";
}

/** The connection as typed. Names follow the engine's `connection.*` fields. */
export interface ConnectionValues {
  kind: string | null;
  bindAddress: string;
  port: string;
  /** Analyzer addresses separated by commas or spaces. */
  allowedPeers: string;
  portPath: string;
  baudRate: string;
  dataBits: string | null;
  parity: string | null;
  stopBits: string | null;
  au: AuValues;
}

export const EMPTY_CONNECTION: ConnectionValues = {
  kind: null,
  bindAddress: "",
  port: "",
  allowedPeers: "",
  portPath: "",
  baudRate: "",
  dataBits: null,
  parity: null,
  stopBits: null,
  au: EMPTY_AU,
};

export function connectionValues(settings: InstrumentConnectionSettings): ConnectionValues {
  switch (settings.kind) {
    case "astm_tcp":
    case "au_tcp":
      return {
        ...EMPTY_CONNECTION,
        kind: settings.kind,
        bindAddress: settings.bindAddress,
        port: String(settings.port),
        allowedPeers: settings.allowedPeers.join(", "),
        au: settings.kind === "au_tcp" ? auValues(settings.au) : EMPTY_AU,
      };
    case "astm_serial":
    case "au_serial":
      return {
        ...EMPTY_CONNECTION,
        kind: settings.kind,
        portPath: settings.portPath,
        baudRate: String(settings.baudRate),
        dataBits: settings.dataBits,
        parity: settings.parity,
        stopBits: settings.stopBits,
        au: settings.kind === "au_serial" ? auValues(settings.au) : EMPTY_AU,
      };
    case "result_file":
      return { ...EMPTY_CONNECTION, kind: settings.kind };
  }
}

/** The settings from validated values; the format checks accepted whole numbers only. */
export function toConnection(values: ConnectionValues): InstrumentConnectionSettings {
  const kind = checked(choiceOf(CONNECTION_KINDS, values.kind));
  switch (kind) {
    case "astm_tcp":
    case "au_tcp": {
      const fields = {
        bindAddress: values.bindAddress.trim(),
        port: Number.parseInt(values.port, 10),
        // REQ-INT-005: only the listed analyzer addresses may connect.
        allowedPeers: values.allowedPeers.split(/[\s,]+/).filter((peer) => peer !== ""),
      };
      return kind === "au_tcp" ? { kind, ...fields, au: toAu(values.au) } : { kind, ...fields };
    }
    case "astm_serial":
    case "au_serial": {
      const fields = {
        portPath: values.portPath.trim(),
        baudRate: Number.parseInt(values.baudRate, 10),
        dataBits: checked(choiceOf(DATA_BITS, values.dataBits)),
        parity: checked(choiceOf(PARITIES, values.parity)),
        stopBits: checked(choiceOf(STOP_BITS, values.stopBits)),
      };
      return kind === "au_serial" ? { kind, ...fields, au: toAu(values.au) } : { kind, ...fields };
    }
    // Files come from a watched folder or are loaded by hand; nothing to connect to (F13).
    case "result_file":
      return { kind: "result_file" };
  }
}

/** The connection in one line, such as `TCP 10.0.0.5:5000, from 10.0.0.21`. */
export function connectionText(settings: InstrumentConnectionSettings): string {
  const prefix =
    settings.kind === "au_tcp" || settings.kind === "au_serial" ? "AU400 / AU640 · " : "";
  if (settings.kind === "astm_tcp" || settings.kind === "au_tcp") {
    return `${prefix}TCP ${settings.bindAddress}:${settings.port}, from ${settings.allowedPeers.join(", ")}`;
  }
  if (settings.kind === "result_file") return "Result files";
  const frame = [
    labelOf(DATA_BITS, settings.dataBits),
    labelOf(PARITIES, settings.parity).charAt(0),
    labelOf(STOP_BITS, settings.stopBits),
  ].join("");
  return `${prefix}Serial ${settings.portPath}, ${settings.baudRate} baud, ${frame}`;
}
