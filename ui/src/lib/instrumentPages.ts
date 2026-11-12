// The two pages that manage one instrument, linked both ways: its connection under Laboratory
// setup, and its service status and result sources under Bench setup.
import type { CommandName } from "@/generated/CommandName";
import type { InstrumentConnectionSettings } from "@/generated/InstrumentConnectionSettings";
import type { SerialDataBits } from "@/generated/SerialDataBits";
import type { SerialParity } from "@/generated/SerialParity";
import type { SerialStopBits } from "@/generated/SerialStopBits";
import { type Choice, labelOf } from "@/lib/choices";

/**
 * Each instrument change restarts the listeners, so their status changes too. Bench setup lists
 * a new or renamed instrument from its readiness.
 */
export const INSTRUMENT_QUERIES: readonly CommandName[] = [
  "list_instruments",
  "list_listener_status",
  "list_instrument_readiness",
];

/** One instrument's panel under Laboratory setup › Instruments: its connection settings. */
export const instrumentConnectionPath = (instrumentId: string) =>
  `/configuration/instruments?open=${instrumentId}`;

/** One instrument's panel under Bench setup › Instruments: its readiness and sources. */
export const benchInstrumentPath = (instrumentId: string) =>
  `/bench/setup/instruments?open=${instrumentId}`;

/** A Bench setup action to start at once when the instrument's panel opens there. */
export type BenchInstrumentStep = "set_instrument_status" | "register_source";

/** The navigation state that asks Bench setup to start `step`. */
export const benchStepState = (step: BenchInstrumentStep) => ({ benchStep: step });

/** The step asked for in the navigation state, if any. */
export function benchStepOf(state: unknown): BenchInstrumentStep | null {
  if (typeof state !== "object" || state === null || !("benchStep" in state)) return null;
  const step = state.benchStep;
  return step === "set_instrument_status" || step === "register_source" ? step : null;
}

export const DATA_BITS: readonly Choice<SerialDataBits>[] = [
  { id: "five", label: "5" },
  { id: "six", label: "6" },
  { id: "seven", label: "7" },
  { id: "eight", label: "8" },
];

export const PARITIES: readonly Choice<SerialParity>[] = [
  { id: "none", label: "None" },
  { id: "odd", label: "Odd" },
  { id: "even", label: "Even" },
];

export const STOP_BITS: readonly Choice<SerialStopBits>[] = [
  { id: "one", label: "1" },
  { id: "two", label: "2" },
];

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
