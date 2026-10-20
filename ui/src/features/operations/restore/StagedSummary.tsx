import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { Facts } from "@/components/Facts";
import { StatusBadge } from "@/components/StatusBadge";
import { Timestamp } from "@/components/Timestamp";
import type { RestoreChainView } from "@/generated/RestoreChainView";
import type { StagedRestoreView } from "@/generated/StagedRestoreView";
import layout from "@/styles/layout.module.css";
import { AnchorCounts } from "../AnchorCounts";
import { findingText, sizeText } from "../labels";

function yesNo(value: boolean, yes: string, no: string) {
  return value ? (
    <StatusBadge tone="success">{yes}</StatusBadge>
  ) : (
    <StatusBadge tone="danger">{no}</StatusBadge>
  );
}

const CHAIN_COLUMNS: readonly DataGridColumn<RestoreChainView>[] = [
  { id: "stream", header: "Audit trail", cell: (chain) => chain.stream, width: 110 },
  { id: "events", header: "Events", cell: (chain) => String(chain.events), width: 90 },
  {
    id: "intact",
    header: "Chain",
    cell: (chain) => yesNo(chain.intact, "Intact", "Broken"),
    width: 110,
  },
  {
    id: "head",
    header: "End",
    cell: (chain) => yesNo(chain.headMatchesManifest, "As recorded", "Not as recorded"),
    width: 170,
  },
];

/** What the engine found when it checked the backup, exactly as it reported it. */
export function StagedSummary({ staged }: { staged: StagedRestoreView }) {
  const facts = [
    { label: "Backup made", value: <Timestamp value={staged.setCreatedAt} /> },
    { label: "App version", value: staged.appVersion },
    { label: "Database size", value: sizeText(staged.databaseBytes) },
    { label: "Stored files", value: String(staged.artifactCount) },
    {
      label: "Database integrity",
      value: yesNo(staged.integrityOk, "Integrity check passed", "Integrity check failed"),
    },
    {
      label: "Lost interval until",
      value: staged.lostIntervalUntil ? (
        <Timestamp value={staged.lostIntervalUntil} />
      ) : (
        "Not reported"
      ),
    },
  ];
  return (
    <>
      <Facts items={facts} />
      <p className={layout.hint}>
        Work recorded after the backup was made is not in it and must be entered again after the
        restore.
      </p>
      <h3 className={layout.subheading}>Audit trails</h3>
      <DataGrid
        label="Audit trails in the backup"
        columns={CHAIN_COLUMNS}
        rows={staged.chains}
        getRowId={(chain) => chain.stream}
      />
      <h3 className={layout.subheading}>Audit anchors</h3>
      <AnchorCounts summary={staged.anchors} />
      <h3 className={layout.subheading}>Findings</h3>
      {staged.findings.length === 0 ? (
        <p>No findings.</p>
      ) : (
        <ul className={layout.list}>
          {staged.findings.map((finding) => (
            <li key={finding.code}>
              {findingText(finding.code)}: {finding.count}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
