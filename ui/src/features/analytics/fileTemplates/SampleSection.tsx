import type { UseQueryResult } from "@tanstack/react-query";
import { Alert } from "@/components/Alert";
import { Facts } from "@/components/Facts";
import { ChoiceInput } from "@/components/FormFields";
import { Inset } from "@/components/Inset";
import { Section } from "@/components/Section";
import type { CommandError } from "@/generated/CommandError";
import type { LayoutSampleOpened } from "@/generated/LayoutSampleOpened";
import type { PreviewLine } from "@/generated/PreviewLine";
import { labelOf } from "@/lib/choices";
import styles from "./fileTemplates.module.css";
import { OpenSample } from "./OpenSample";
import { SamplePreview } from "./SamplePreview";
import { DELIMITERS, ENCODINGS, TRIM_FIELDS } from "./templateLabels";

/** Raw lines shown before a delimiter is chosen, to see which one the file uses. */
const RAW_LINES = 5;

interface SampleSectionProps {
  sample: LayoutSampleOpened | null;
  onOpened: (sample: LayoutSampleOpened) => void;
  /** The first lines as cells, once the encoding and delimiter are chosen. */
  preview: UseQueryResult<PreviewLine[], CommandError> | null;
  /** The header rows chosen so far, to warn when their widths differ. */
  header: string[][] | null;
}

/**
 * Step 2: a sample export, how its text is read, and which line is the header (F3, F6). The
 * sample stays on this page only; the template keeps only the header.
 */
export function SampleSection({ sample, onOpened, preview, header }: SampleSectionProps) {
  return (
    <Section title="2. Sample export">
      <p>
        Open an export written with the laboratory's export settings. It is held only while this
        page is open and is never stored; the template keeps only its header.
      </p>
      <OpenSample
        label={sample ? "Open another sample" : "Open a sample export"}
        onOpened={onOpened}
      />
      {sample && <SampleText sample={sample} />}
      {preview && <SamplePreview preview={preview} header={header} />}
    </Section>
  );
}

function SampleText({ sample }: { sample: LayoutSampleOpened }) {
  return (
    <>
      {sample.bomEncoding ? (
        <Facts
          items={[
            {
              label: "Encoding",
              value: `${labelOf(ENCODINGS, sample.bomEncoding)}, set by the mark at the start of the file`,
            },
          ]}
        />
      ) : (
        <ChoiceInput name="layout.encoding" label="Encoding" options={ENCODINGS} isRequired />
      )}
      {!sample.validUtf8 && (
        <Alert tone="warning" title="The sample is not valid UTF-8">
          Choose Windows-1252 if the software writes it, or save the export as UTF-8.
        </Alert>
      )}
      <ChoiceInput name="layout.delimiter" label="Delimiter" options={DELIMITERS} isRequired />
      <ChoiceInput
        name="layout.trimFields"
        label="Spaces around values"
        options={TRIM_FIELDS}
        isRequired
      />
      <Inset>
        <figure className={styles.raw}>
          <figcaption>First lines as written</figcaption>
          <pre>{sample.lines.slice(0, RAW_LINES).join("\n")}</pre>
        </figure>
      </Inset>
    </>
  );
}
