import { useLocation, useNavigate } from "react-router";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import type { BillingClassMappingVersionSummary } from "@/generated/BillingClassMappingVersionSummary";
import { usePersonText } from "@/lib/person";
import { mappingStatus } from "../labels";
import { classMappingPath } from "../paths";
import { EffectivePeriod } from "./EffectivePeriod";

type Version = BillingClassMappingVersionSummary;

function columns(person: (userId: string, display?: string) => string) {
  const approver = (row: Version) =>
    row.approvedBy === null ? "Not yet" : person(row.approvedBy, row.approvedByDisplay);
  const list: readonly DataGridColumn<Version>[] = [
    {
      id: "version",
      header: "Version",
      cell: (row) => `Version ${row.version}`,
      sortValue: (row) => row.version,
      identifier: true,
      width: 110,
    },
    {
      id: "status",
      header: "Status",
      cell: (row) => {
        const status = mappingStatus(row);
        return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
      },
      width: 210,
    },
    { id: "effective", header: "Effective", cell: (row) => <EffectivePeriod version={row} /> },
    {
      id: "drafted",
      header: "Drafted by",
      cell: (row) => person(row.createdBy, row.createdByDisplay),
    },
    { id: "approved", header: "Approved by", cell: approver },
  ];
  return list;
}

interface MappingVersionsProps {
  versions: readonly Version[];
  /** The version whose content the page shows. */
  shownId: string | undefined;
}

/**
 * Every version of the class mapping, newest first, with its status, effective period, and
 * who drafted and approved it (the engine's display names). Opening one shows it
 * (`?version=<number>`), replacing the history entry and keeping the navigation state.
 */
export function MappingVersions({ versions, shownId }: MappingVersionsProps) {
  const navigate = useNavigate();
  const { state } = useLocation();
  const person = usePersonText();
  const newestFirst = [...versions].sort((a, b) => b.version - a.version);
  return (
    <DataGrid
      label="Versions"
      columns={columns(person)}
      rows={newestFirst}
      getRowId={(row) => row.versionId}
      openRowId={shownId ?? null}
      onRowAction={(row) => void navigate(classMappingPath(row.version), { replace: true, state })}
    />
  );
}
