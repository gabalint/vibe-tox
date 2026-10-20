import type { ReactNode } from "react";
import { useNavigate } from "react-router";
import { Button } from "@/components/Button";
import type { GridRowActions } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { ErrorState } from "@/components/ErrorState";
import { LoadingState } from "@/components/LoadingState";
import { Toolbar } from "@/components/Toolbar";
import type { SubjectSearch } from "@/generated/SubjectSearch";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { SubjectPicker } from "../SubjectPicker";
import { newOrderPath, subjectName } from "../subjects";
import { SEARCH_SUBJECTS } from "./contract";
import { SEARCH_LIMIT } from "./PatientSearch";
import styles from "./PatientsPage.module.css";

const PAGE_SIZE = SEARCH_LIMIT - 1;

interface SearchResultsProps {
  criteria: SubjectSearch;
  toolbar: (count: ReactNode) => ReactNode;
  filtered: boolean;
  onClear: () => void;
  onPage: (offset: number) => void;
  onChoose?: (subject: SubjectSummary) => void;
  registrationPath?: string;
}

/** One audited page, with a lookahead row so Next only appears when more patients exist. */
export function SearchResults({
  criteria,
  filtered,
  onClear,
  onPage,
  toolbar,
  onChoose,
  registrationPath,
}: SearchResultsProps) {
  const rowActions = usePatientOrderActions(Boolean(onChoose));
  const results = useCommandQuery<SubjectSummary[]>(
    SEARCH_SUBJECTS,
    { input: criteria },
    { keepPrevious: true },
  );
  const offset = criteria.offset ?? 0;
  const subjects = results.data?.slice(0, PAGE_SIZE) ?? [];
  const count = resultCount(results, offset, filtered);
  return (
    <>
      {toolbar(count)}
      <div className={styles.results} aria-busy={results.isFetching}>
        {results.isPending ? (
          <LoadingState label="Loading patients…" />
        ) : results.isError ? (
          <ErrorState
            title="Patients could not be loaded"
            error={results.error}
            onRetry={() => void results.refetch()}
          />
        ) : (
          <>
            {subjects.length > 0 ? (
              <SubjectPicker
                label="Patients"
                listLabel="Patients"
                subjects={subjects}
                rowActions={rowActions}
                onChoose={onChoose}
                paged
              />
            ) : (
              <NoPatients
                filtered={filtered}
                offset={offset}
                onClear={onClear}
                registrationPath={registrationPath}
              />
            )}
            <div className={styles.pagination}>
              <Toolbar>
                <Button
                  isDisabled={offset === 0 || results.isFetching}
                  onPress={() => onPage(Math.max(0, offset - PAGE_SIZE))}
                >
                  Previous
                </Button>
                <Button
                  isDisabled={(results.data?.length ?? 0) <= PAGE_SIZE || results.isFetching}
                  onPress={() => onPage(offset + PAGE_SIZE)}
                >
                  Next
                </Button>
              </Toolbar>
            </div>
          </>
        )}
      </div>
    </>
  );
}

/** Patient selection stays within its current task; directory ordering needs its own authority. */
function usePatientOrderActions(
  selectingPatient: boolean,
): GridRowActions<SubjectSummary> | undefined {
  const navigate = useNavigate();
  const canEnterOrders = useHasCapability("enter_orders");
  if (selectingPatient || !canEnterOrders) return undefined;
  // Navigation mirrors the distinct order-entry capability; the engine rechecks every command.
  return {
    label: subjectName,
    hasItems: () => true,
    items: (subject) => [
      {
        id: "new_order",
        label: "New order",
        onAction: () => navigate(newOrderPath(subject.subjectId)),
      },
    ],
  };
}

function resultCount(
  result: { isSuccess: boolean; isFetching: boolean; data?: readonly SubjectSummary[] },
  offset: number,
  filtered: boolean,
) {
  if (!result.isSuccess || !result.data) return null;
  if (result.isFetching) return "Updating patients…";
  return pageCount(result.data.length, offset, filtered);
}

function pageCount(length: number, offset: number, filtered: boolean): string {
  if (length === 0) return offset > 0 ? "0 patients on this page" : "0 patients";
  if (offset > 0 || length > PAGE_SIZE) {
    return `Patients ${offset + 1}–${offset + Math.min(length, PAGE_SIZE)}`;
  }
  return `${length} ${filtered ? "matching " : ""}${length === 1 ? "patient" : "patients"}`;
}

function NoPatients({
  filtered,
  offset,
  onClear,
  registrationPath = "/patients/register",
}: Omit<SearchResultsProps, "criteria" | "onPage" | "toolbar"> & { offset: number }) {
  const navigate = useNavigate();
  const canEnterOrders = useHasCapability("enter_orders");
  if (offset > 0)
    return (
      <EmptyState
        title="No more patients"
        description="Return to the previous page or start a new search."
      />
    );
  if (filtered)
    return (
      <EmptyState
        title="No matching patients"
        description="Try another name or identifier, or show all patients."
        action={<Button onPress={onClear}>Clear search</Button>}
      />
    );
  return (
    <EmptyState
      title="No patients yet"
      description={
        canEnterOrders
          ? "Register a patient to start their laboratory record."
          : "No patient records are available for identity review."
      }
      action={
        canEnterOrders && (
          <Button onPress={() => navigate(registrationPath)}>Register patient</Button>
        )
      }
    />
  );
}
