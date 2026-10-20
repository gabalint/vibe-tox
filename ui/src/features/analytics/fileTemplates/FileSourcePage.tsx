import { useState } from "react";
import { useParams } from "react-router";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { Section } from "@/components/Section";
import { StatusBadge } from "@/components/StatusBadge";
import type { CheckFileLayoutInput } from "@/generated/CheckFileLayoutInput";
import type { FileLayout } from "@/generated/FileLayout";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { LayoutCheck } from "@/generated/LayoutCheck";
import type { SourceRegistrationView } from "@/generated/SourceRegistrationView";
import { useCommandMutation, useCommandQuery } from "@/lib/commands";
import { ApproveSource } from "../bench/ApproveSource";
import {
  CHECK_FILE_LAYOUT,
  LIST_INSTRUMENT_READINESS,
  LIST_SOURCE_REGISTRATIONS,
} from "../contract";
import { registrationStatus } from "../labels";
import { BENCH_INSTRUMENTS, benchInstrumentPath, fileSourcePath } from "../paths";
import { useAnalyteName } from "../useAnalyteName";
import { LayoutCheckResult } from "./LayoutCheckResult";
import { OpenSample } from "./OpenSample";
import { TemplateFacts } from "./TemplateFacts";

/**
 * One file source's template (result files brief §7; REQ-ANA-003): what it says, a check
 * with a file of the reader's choice, and, for a draft, approval by someone other than its
 * author, or by its author where the laboratory's separation-of-duties policy allows it. The
 * check writes nothing.
 */
export function FileSourcePage() {
  const { instrumentId = "", registrationId = "" } = useParams();
  const [checked, setChecked] = useState(false);
  const sources = useCommandQuery<SourceRegistrationView[]>(LIST_SOURCE_REGISTRATIONS, {
    input: { instrumentId },
  });
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const instrumentName = instruments.data?.find((row) => row.instrumentId === instrumentId)?.name;
  const source = sources.data?.find((row) => row.registrationId === registrationId);
  const title = source ? `${source.senderName} ${source.senderVersion}` : "File template";
  const status = source && registrationStatus(source.state);
  return (
    <Page>
      <PageHeader
        title={title}
        description={instrumentName && `File template of ${instrumentName}.`}
        status={status && <StatusBadge tone={status.tone}>{status.label}</StatusBadge>}
        breadcrumbs={[
          BENCH_INSTRUMENTS,
          { label: instrumentName ?? "Instrument", href: benchInstrumentPath(instrumentId) },
          { label: title, href: fileSourcePath(instrumentId, registrationId) },
        ]}
        actions={
          source && (
            <ApproveSource source={source}>
              <p>
                Files from {instrumentName ?? "this instrument"} will be imported with this template
                and its analyte map.
                {checked
                  ? " You checked it with a file on this page."
                  : " You have not checked it with a file on this page."}{" "}
                The person who drafted it cannot approve it unless your laboratory's
                separation-of-duties policy allows self-approval.
              </p>
            </ApproveSource>
          )
        }
      />
      <QueryContent query={sources} what="file template">
        {() =>
          source?.fileLayout ? (
            <>
              <TemplateFacts source={source} layout={source.fileLayout} />
              <SourceCheck
                source={source}
                layout={source.fileLayout}
                onChecked={() => setChecked(true)}
              />
            </>
          ) : (
            <EmptyState
              title="File template not found"
              description="Open it again from the instrument in Bench setup."
            />
          )
        }
      </QueryContent>
    </Page>
  );
}

interface SourceCheckProps {
  source: SourceRegistrationView;
  layout: FileLayout;
  onChecked: () => void;
}

/** Reads a file of the reader's choice with this template, as an import would. */
function SourceCheck({ source, layout, onChecked }: SourceCheckProps) {
  const check = useCommandMutation<LayoutCheck, { input: CheckFileLayoutInput }>(
    CHECK_FILE_LAYOUT,
    { invalidates: [] },
  );
  const analyteName = useAnalyteName();
  const checkWith = (sampleHandle: string) =>
    check.mutate(
      { input: { sampleHandle, layout, testCodes: source.testCodes } },
      { onSuccess: onChecked },
    );
  return (
    <Section title="Check with a file">
      <p>
        Open an export, such as one from a batch also checked in the instrument's software. It is
        read with this template exactly as an import would read it, and nothing is stored. Whether
        each label is on an open run is known only when a file is imported.
      </p>
      <OpenSample
        label={check.data ? "Check with another file" : "Check with a file"}
        onOpened={(sample) => checkWith(sample.sampleHandle)}
      />
      {check.isPending && <LoadingState label="Checking the file…" />}
      {check.error && <ErrorState title="The check did not run" error={check.error} />}
      {check.data && <LayoutCheckResult check={check.data} analyteName={analyteName} />}
    </Section>
  );
}
