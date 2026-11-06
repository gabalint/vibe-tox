// The form values of an MLLP listener (ticket #4555 step 4b): where the LIS listens and which
// partner addresses may connect, written in one field as "192.168.1.50 (Hospital B interface
// engine), 192.168.1.51". The engine checks every value again: lab-network addresses only.
import type { AllowedPartner } from "@/generated/AllowedPartner";
import type { CharacterSet } from "@/generated/CharacterSet";
import type { MllpAcknowledgement } from "@/generated/MllpAcknowledgement";
import type { MllpListener } from "@/generated/MllpListener";

export interface ListenerValues {
  address: string;
  port: string;
  /** Every allowed partner address, each with its note in brackets, comma separated. */
  allowedPartners: string;
  acknowledgement: MllpAcknowledgement;
  characterSet: CharacterSet;
  idleTimeoutSeconds: string;
}

/** An address, then its note in brackets if any. */
const PARTNER = /([^\s,()]+)\s*(?:\(([^)]*)\))?/g;

/** The allowed partners as one line, as the field shows them. */
export const partnersText = (partners: readonly AllowedPartner[]) =>
  partners.map(({ address, note }) => (note ? `${address} (${note})` : address)).join(", ");

/** The allowed partners in the field's text. */
export const toPartners = (text: string): AllowedPartner[] =>
  [...text.matchAll(PARTNER)].map(([, address = "", note]) => ({
    address,
    note: note?.trim() ?? "",
  }));

export function toListenerValues(listener: MllpListener | null | undefined): ListenerValues {
  return {
    address: listener?.address ?? "",
    port: listener ? String(listener.port) : "",
    allowedPartners: partnersText(listener?.allowedPartners ?? []),
    acknowledgement: listener?.acknowledgement ?? "application",
    characterSet: listener?.characterSet ?? "utf-8",
    idleTimeoutSeconds: String(listener?.idleTimeoutSeconds ?? 300),
  };
}

/** The listener as entered; the engine trims and checks it. */
export function toListener(values: ListenerValues): MllpListener {
  return {
    address: values.address.trim(),
    port: Number(values.port),
    allowedPartners: toPartners(values.allowedPartners),
    acknowledgement: values.acknowledgement,
    characterSet: values.characterSet,
    idleTimeoutSeconds: Number(values.idleTimeoutSeconds),
  };
}
