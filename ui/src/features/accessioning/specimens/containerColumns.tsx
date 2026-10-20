import type { DataGridColumn } from "@/components/DataGrid";
import { StatusBadge } from "@/components/StatusBadge";
import type { ContainerView } from "@/generated/ContainerView";
import styles from "../accessioning.module.css";
import { containerStateLabel, labelJobStatus, volumeText } from "./labels";

/** The latest label job of a container, or that none was requested. */
export function LatestLabelJob({ container }: { container: ContainerView }) {
  const job = container.labelJobs.at(-1);
  if (!job) return "No label requested";
  const status = labelJobStatus(job);
  return <StatusBadge tone={status.tone}>{status.label}</StatusBadge>;
}

/** Open container holds, which block aliquots, disposal and testing. */
export function HoldCount({ count }: { count: number }) {
  if (count === 0) return "None";
  return <StatusBadge tone="hold">{`${count} open`}</StatusBadge>;
}

/**
 * Container columns, with two identifiers on every row: the label and the accession number
 * (Clinical display rules). An aliquot names its parent by the parent's label.
 */
export function containerColumns(
  accessionNumber: string,
  labelOf: (containerId: string) => string,
): DataGridColumn<ContainerView>[] {
  return [
    {
      id: "label",
      header: "Barcode",
      cell: (container) => <span className={styles.code}>{container.labelCode}</span>,
      sortValue: (container) => container.labelCode,
      width: 130,
    },
    {
      id: "accession",
      header: "Accession",
      cell: () => <span className={styles.code}>{accessionNumber}</span>,
      width: 120,
    },
    {
      id: "kind",
      header: "Kind",
      cell: (container) =>
        container.parentContainerId === null
          ? "Primary"
          : `Aliquot of ${labelOf(container.parentContainerId)}`,
      width: 170,
    },
    {
      id: "state",
      header: "State",
      cell: (container) => containerStateLabel(container.state),
      width: 110,
    },
    { id: "volume", header: "Volume", cell: volumeText },
    {
      id: "location",
      header: "Location",
      cell: (container) =>
        container.locationChronologyUnresolved
          ? "Legacy chronology unresolved"
          : (container.location?.name ?? "Not recorded"),
    },
    {
      id: "holds",
      header: "Holds",
      cell: (container) =>
        container.openHolds.length === 0 ? 0 : <HoldCount count={container.openHolds.length} />,
      sortValue: (container) => container.openHolds.length,
      align: "end",
      width: 100,
    },
    {
      id: "labelJob",
      header: "Label printing",
      cell: (container) => <LatestLabelJob container={container} />,
      width: 250,
    },
  ];
}
