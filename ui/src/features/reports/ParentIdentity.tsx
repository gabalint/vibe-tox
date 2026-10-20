import type { UseQueryResult } from "@tanstack/react-query";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import type { CommandError } from "@/generated/CommandError";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { IdentityBanner } from "@/lib/IdentityBanner";

/** A record that names the patient and the specimen: a report or a specimen's workspace. */
interface Identified {
  subject: SubjectSummary;
  accessionNumber: string;
}

/**
 * The identity banner of a record whose own view carries no patient (a candidate, a
 * revision), from its parent's read (the specimen's workspace, the report). The parent read
 * is an audited access, so render this only on a page the person explicitly opened.
 */
export function ParentIdentity({ parent }: { parent: UseQueryResult<Identified, CommandError> }) {
  if (parent.isSuccess) {
    return (
      <IdentityBanner
        subject={parent.data.subject}
        specimen={{ accessionNumber: parent.data.accessionNumber }}
      />
    );
  }
  if (parent.isError) {
    return (
      <ErrorState
        title="The patient could not be loaded"
        error={parent.error}
        onRetry={() => void parent.refetch()}
      />
    );
  }
  return <LoadingState label="Loading patient…" />;
}
