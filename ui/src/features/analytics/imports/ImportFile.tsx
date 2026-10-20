import { Link } from "react-router";
import { Facts } from "@/components/Facts";
import { Section } from "@/components/Section";
import type { ImportAttemptDetail } from "@/generated/ImportAttemptDetail";
import type { ImportFileView } from "@/generated/ImportFileView";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { useCommandQuery } from "@/lib/commands";
import { usePersonText } from "@/lib/person";
import styles from "@/styles/layout.module.css";
import { LIST_SOURCE_REGISTRATIONS } from "../contract";
import { fileSourcePath } from "../paths";

/** How the file arrived and who loaded it (F16). */
function useOriginText(): (file: ImportFileView) => string {
  const person = usePersonText();
  return (file) =>
    file.origin === "folder" || file.loadedBy === null
      ? "Picked up from the watched folder"
      : `Loaded by hand by ${person(file.loadedBy, file.loadedByDisplay)}`;
}

/**
 * The file an import attempt read, and the approved source it was read with (result files
 * brief §7): origin, file name, size and digest, and the template version, which opens on
 * its own page.
 */
export function ImportFile({ attempt }: { attempt: ImportAttemptDetail }) {
  const origin = useOriginText();
  const file = attempt.file;
  if (file === null) return null;
  return (
    <Section title="File">
      <Facts
        items={[
          { label: "Origin", value: origin(file) },
          { label: "File name", value: file.fileName },
          { label: "Size", value: `${file.byteLen.toLocaleString("en-US")} bytes` },
          {
            label: "SHA-256",
            value: <span className={styles.code}>{file.sha256 ?? "Not computed"}</span>,
          },
          { label: "Template", value: <TemplateUsed attempt={attempt} /> },
        ]}
      />
    </Section>
  );
}

/** The registration version the attempt used, by its declared software name and version. */
function TemplateUsed({ attempt }: { attempt: ImportAttemptDetail }) {
  const sources = useCommandQuery<SourceRegistrationView[]>(
    LIST_SOURCE_REGISTRATIONS,
    { input: { instrumentId: attempt.instrumentId } },
    { enabled: attempt.registrationId !== null },
  );
  if (attempt.registrationId === null) return "None: no approved template when it was read";
  const source = sources.data?.find((row) => row.registrationId === attempt.registrationId);
  if (source === undefined) return sources.isPending ? "Loading…" : "Not found";
  const name = `${source.senderName} ${source.senderVersion}`;
  if (source.fileLayout === null) return name;
  return <Link to={fileSourcePath(attempt.instrumentId, source.registrationId)}>{name}</Link>;
}
