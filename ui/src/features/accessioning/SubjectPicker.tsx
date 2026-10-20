import { useMemo } from "react";
import { DataGrid, type DataGridColumn, type GridRowActions } from "@/components/DataGrid";
import type { SubjectSummary } from "@/generated/SubjectSummary";
import { useFormatBirthDate } from "@/lib/format";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { IdentityHoldBadge } from "./IdentityHoldBadge";
import { identifierText, orNotRecorded, patientPath, subjectName } from "./subjects";

/**
 * Two identifiers on every row (REQ-IDN-001): the name, the birth date with the month as
 * text and the age, and the active identifiers.
 */
function subjectColumns(
  formatBirthDate: (birthDate: string) => string,
): DataGridColumn<SubjectSummary>[] {
  return [
    { id: "name", header: "Name", cell: subjectName, sortValue: subjectName, identifier: true },
    {
      id: "birthDate",
      header: "Birth date",
      cell: (subject) => formatBirthDate(subject.birthDate),
      sortValue: (subject) => subject.birthDate,
      width: 170,
    },
    {
      id: "sex",
      header: "Administrative sex",
      cell: (subject) => orNotRecorded(subject.administrativeSex),
      width: 160,
    },
    {
      id: "identifiers",
      header: "Identifiers",
      cell: (subject) => identifierText(subject.identifiers),
    },
    {
      id: "identity",
      header: "Identity",
      cell: (subject) => <IdentityHoldBadge count={subject.openIdentityHolds} />,
      sortValue: (subject) => subject.openIdentityHolds,
      width: 200,
    },
  ];
}

const getRowId = (subject: SubjectSummary) => subject.subjectId;

interface SubjectPickerProps {
  /** Accessible name of the grid. */
  label: string;
  /** The `h1` of the screen showing the grid, for the patient page's breadcrumb. */
  listLabel: string;
  subjects: readonly SubjectSummary[];
  rowActions?: GridRowActions<SubjectSummary>;
  /** A directory page is already ordered by the engine; do not sort only its visible slice. */
  paged?: boolean;
  /** Task selection keeps the patient in the current order or registration flow. */
  onChoose?: (subject: SubjectSummary) => void;
}

/** Patients in a grid; a click or Enter on a row opens the patient's record, an audited read. */
export function SubjectPicker({
  label,
  listLabel,
  subjects,
  rowActions,
  paged,
  onChoose,
}: SubjectPickerProps) {
  const formatBirthDate = useFormatBirthDate();
  const columns = useMemo(() => {
    const columns = subjectColumns(formatBirthDate);
    return paged ? columns.map((column) => ({ ...column, sortValue: undefined })) : columns;
  }, [formatBirthDate, paged]);
  const open = useOpenRecordPage(patientPath, listLabel);
  return (
    <DataGrid
      label={label}
      columns={columns}
      rows={subjects}
      getRowId={getRowId}
      onRowAction={(subject, order) =>
        onChoose ? onChoose(subject) : open(subject.subjectId, order)
      }
      rowActions={rowActions}
      fill={paged}
    />
  );
}
