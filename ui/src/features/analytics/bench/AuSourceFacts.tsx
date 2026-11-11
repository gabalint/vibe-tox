import { Facts } from "@/components/Facts";
import type { TextFormat } from "@/components/FormFields";
import type { AuConfig } from "@/generated/AuConfig";
import { labelOf } from "@/lib/choices";
import { REVISION_INDICATORS } from "../labels";

/** An AU test code as the engine accepts it (REQ-INT-001). */
export const AU_TEST_CODE: TextFormat = {
  pattern: /^(0[1-9]|[1-9]\d)$/,
  message: "Use a two-digit code from 01 to 99.",
  description: "The analyzer's two-digit test number, 01–99.",
};

const hex = (bytes: number[]) =>
  bytes.map((byte) => byte.toString(16).padStart(2, "0").toUpperCase()).join(" ");

/** The AU wire settings in one line, such as `Class B · start 02 · end 03 · BCC · …`. */
export function auWireText({ framing, format }: AuConfig): string {
  return [
    `Class ${framing.class.toUpperCase()}`,
    `start ${hex(framing.start)}`,
    `end ${hex(framing.end)}`,
    framing.bcc ? "BCC" : "no BCC",
    `sample ID width ${format.sample_id_digits}`,
    `result width ${format.result_digits}`,
  ].join(" · ");
}

/**
 * What an AU source takes as given rather than asks: the connection's wire settings, which
 * the source restates exactly, barcode matching and no correction marker.
 */
export function AuSourceFacts({ protocol }: { protocol: AuConfig }) {
  return (
    <Facts
      items={[
        {
          label: "AU wire settings",
          value: `From the analyzer's connection: ${auWireText(protocol)}`,
        },
        { label: "Sample ID is read from", value: "The sample ID of the AU message" },
        { label: "Corrected results", value: labelOf(REVISION_INDICATORS, "none") },
      ]}
    />
  );
}
