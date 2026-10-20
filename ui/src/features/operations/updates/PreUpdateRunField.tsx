import { useMemo } from "react";
import { Link } from "react-router";
import { Button } from "@/components/Button";
import { ErrorState } from "@/components/ErrorState";
import { ChoiceInput, RecordPickerInput } from "@/components/FormFields";
import { LoadingState } from "@/components/LoadingState";
import { useFormatTime } from "@/lib/format";
import { useBackupRuns } from "../backups/useBackupLists";
import { RequestUpdateBackup } from "./RequestUpdateBackup";

/** `Select` holds up to this many fixed options; above it the field is a `RecordPicker`. */
const SELECT_UP_TO = 15;

interface PreUpdateRunFieldProps {
  /** The form path of the chosen run's ID. */
  name: string;
}

/**
 * Chooses the verified backup taken before the update from the runs the engine lists, newest
 * first, so no run ID is typed (REQ-OPS-006). The engine still checks the run and how recent
 * it is when the approval is recorded.
 */
export function PreUpdateRunField({ name }: PreUpdateRunFieldProps) {
  const { query, rows, hasMore } = useBackupRuns();
  const formatTime = useFormatTime();
  const options = useMemo(
    () =>
      rows
        .filter((run) => run.updateApprovalEligible)
        .map((run) => ({
          id: run.runId,
          label: formatTime(run.finishedAt ?? run.requestedAt),
          description: `Verified on ${run.copies
            .filter((copy) => copy.state === "succeeded" && !copy.pruned)
            .map((copy) => copy.targetLabel)
            .join(", ")}`,
        })),
    [rows, formatTime],
  );
  // The field is always mounted, so "required" holds while the runs load or none qualifies.
  return (
    <>
      <RunChoice name={name} options={options} />
      <p>
        From <Link to="/operations/backups/runs">Backups → Runs</Link>. Scheduled backups are
        excluded because automatic retention can remove them.
      </p>
      {query.isPending && <LoadingState label="Loading the backup runs…" />}
      {query.isError && (
        <ErrorState
          title="The backup runs could not be loaded"
          error={query.error}
          onRetry={() => void query.refetch()}
        />
      )}
      {query.isSuccess && options.length === 0 && (
        <p>
          No eligible backup in the loaded history. Request one below and wait for verification.
        </p>
      )}
      <RequestUpdateBackup>
        <Button isPending={query.isFetching} onPress={() => void query.refetch()}>
          Refresh backup list
        </Button>
        {hasMore && (
          <Button
            isPending={query.isFetchingNextPage}
            isDisabled={query.isFetching}
            onPress={() => void query.fetchNextPage()}
          >
            Load older backup runs
          </Button>
        )}
      </RequestUpdateBackup>
    </>
  );
}

interface RunChoiceProps {
  name: string;
  options: readonly { id: string; label: string; description: string }[];
}

function RunChoice({ name, options }: RunChoiceProps) {
  const label = "Backup before this update";
  const description = "Verified manual or pre-update backups from the last 24 hours.";
  if (options.length > SELECT_UP_TO) {
    return (
      <RecordPickerInput
        name={name}
        label={label}
        description={description}
        options={options}
        isRequired
      />
    );
  }
  return (
    <ChoiceInput
      name={name}
      label={label}
      description={description}
      options={options.map((option) => ({
        id: option.id,
        label: `${option.label}. ${option.description}`,
      }))}
      isRequired
    />
  );
}
