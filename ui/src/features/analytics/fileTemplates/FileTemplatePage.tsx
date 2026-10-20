import type { ReactNode } from "react";
import { useParams } from "react-router";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Page, PageHeader } from "@/components/Page";
import type { InstrumentReadinessView } from "@/generated/InstrumentReadinessView";
import type { SessionInfo } from "@/generated/SessionInfo";
import { CURRENT_SESSION, useCommandQuery } from "@/lib/commands";
import { LIST_INSTRUMENT_READINESS, MANAGE_CONFIGURATION } from "../contract";
import { BENCH_INSTRUMENTS, benchInstrumentPath, fileTemplatePath } from "../paths";
import { TemplateForm } from "./TemplateForm";

const TITLE = "New file template";

/**
 * Drafts the file template of a result-file instrument (result files brief §7), for people
 * with `manage_configuration`. Opened from the instrument's panel in Bench setup.
 */
export function FileTemplatePage() {
  const { instrumentId = "" } = useParams();
  const session = useCommandQuery<SessionInfo | null>(CURRENT_SESSION);
  const instruments = useCommandQuery<InstrumentReadinessView[]>(LIST_INSTRUMENT_READINESS);
  const name = instruments.data?.find((row) => row.instrumentId === instrumentId)?.name;
  const canDraft = session.data?.capabilities.includes(MANAGE_CONFIGURATION) ?? false;
  let body: ReactNode = <TemplateForm instrumentId={instrumentId} />;
  if (session.isPending || instruments.isPending) {
    body = <LoadingState label="Loading the instrument…" />;
  } else if (!canDraft) {
    body = (
      <EmptyState
        title="You cannot draft file templates"
        description="Drafting a file template needs the manage configuration capability. A laboratory administrator can change roles."
      />
    );
  } else if (instruments.isError) {
    body = (
      <ErrorState
        title="The instrument could not be loaded"
        error={instruments.error}
        onRetry={() => void instruments.refetch()}
      />
    );
  } else if (name === undefined) {
    body = (
      <EmptyState title="Instrument not found" description="Open it again from Bench setup." />
    );
  }
  return (
    <Page>
      <PageHeader
        title={TITLE}
        description={
          name &&
          `For ${name}. Describe the export exactly as the software writes it; a file whose header differs is not imported.`
        }
        breadcrumbs={[
          BENCH_INSTRUMENTS,
          { label: name ?? "Instrument", href: benchInstrumentPath(instrumentId) },
          { label: TITLE, href: fileTemplatePath(instrumentId) },
        ]}
      />
      {body}
    </Page>
  );
}
