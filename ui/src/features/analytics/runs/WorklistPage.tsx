import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid, type DataGridColumn } from "@/components/DataGrid";
import { ListCount } from "@/components/ListLimit";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Select } from "@/components/Select";
import { Timestamp } from "@/components/Timestamp";
import { Toolbar } from "@/components/Toolbar";
import type { CatalogVersionSummary } from "@/generated/CatalogVersionSummary";
import type { MethodView } from "@/generated/MethodView";
import type { RunDetail } from "@/generated/RunDetail";
import type { WorklistEntry } from "@/generated/WorklistEntry";
import { useCommandQuery } from "@/lib/commands";
import styles from "@/styles/layout.module.css";
import { LIST_METHOD_VERSIONS, LIST_METHODS, LIST_WORKLIST } from "../contract";
import { runPath } from "../paths";
import { CreateRunDialog } from "./CreateRunDialog";
import { PrepBatchSection } from "./PrepBatchSection";
import runs from "./runs.module.css";

const COLUMNS: readonly DataGridColumn<WorklistEntry>[] = [
  {
    id: "container",
    header: "Container",
    cell: (row) => <span className={styles.code}>{row.labelCode}</span>,
    sortValue: (row) => row.labelCode,
    width: 160,
  },
  {
    id: "accession",
    header: "Accession",
    cell: (row) => row.accessionNumber,
    sortValue: (row) => row.accessionNumber,
    width: 160,
  },
  { id: "type", header: "Specimen type", cell: (row) => row.specimenTypeCode, width: 140 },
  { id: "tests", header: "Tests", cell: (row) => row.orderables.join(", ") },
  {
    id: "received",
    header: "Received",
    cell: (row) => <Timestamp value={row.receivedAt} />,
    sortValue: (row) => row.receivedAt,
    width: 200,
  },
];

/**
 * Accepted containers waiting for one method version (REQ-ANA-001), the start of a run for
 * them, and a preparation batch for the method version. The method and version stay in the
 * address. No patient details are shown; each row names the container and its accession.
 * Accessioning adds containers while the screen is open, so the list says when it was read.
 * The waiting containers are the screen's one growing grid; the batch's steps stay few.
 */
export function WorklistPage() {
  const navigate = useNavigate();
  const [isCreating, setCreating] = useState(false);
  const [params, setParams] = useSearchParams();
  const methodId = params.get("method");
  const versionId = params.get("version");
  const worklist = useCommandQuery<WorklistEntry[]>(
    LIST_WORKLIST,
    { input: { methodVersionId: versionId } },
    { enabled: Boolean(methodId && versionId) },
  );
  const choose = (method: string | null, version: string | null) => {
    const next = new URLSearchParams();
    if (method) next.set("method", method);
    if (method && version) next.set("version", version);
    setParams(next);
  };
  return (
    <Page layout="collection">
      <PageHeader
        title="Worklist"
        actions={
          methodId &&
          versionId && (
            <Button variant="primary" onPress={() => setCreating(true)}>
              Create run
            </Button>
          )
        }
      />
      <Toolbar
        variant="collection"
        count={
          methodId &&
          versionId &&
          worklist.data && (
            <ListCount
              shown={worklist.data.length}
              noun="waiting containers"
              one="waiting container"
            />
          )
        }
      >
        <MethodChoice value={methodId} onChange={(method) => choose(method, null)} />
        {methodId && (
          <VersionChoice
            methodId={methodId}
            value={versionId}
            onChange={(version) => choose(methodId, version)}
          />
        )}
        {methodId && versionId && <RefreshControl query={worklist} />}
      </Toolbar>
      {isCreating && methodId && versionId && (
        <CreateRunDialog
          methodVersionId={versionId}
          onCreated={(run: RunDetail) => navigate(runPath(run.runId))}
          onClose={() => setCreating(false)}
        />
      )}
      {methodId && versionId && (
        <>
          <Worklist query={worklist} />
          <div className={runs.preparation}>
            <PrepBatchSection key={versionId} methodVersionId={versionId} />
          </div>
        </>
      )}
    </Page>
  );
}

interface ChoiceProps {
  value: string | null;
  onChange: (value: string | null) => void;
}

function MethodChoice({ value, onChange }: ChoiceProps) {
  const methods = useCommandQuery<MethodView[]>(LIST_METHODS);
  return (
    <QueryContent query={methods} what="methods">
      {(rows) => (
        <Select
          layout="inline"
          label="Method"
          name="method"
          options={rows.map((method) => ({
            id: method.id,
            label: `${method.name} (${method.code})`,
          }))}
          value={value}
          onChange={onChange}
        />
      )}
    </QueryContent>
  );
}

/** Only approved versions can be run; the engine checks again when the run is created. */
function VersionChoice({ methodId, value, onChange }: ChoiceProps & { methodId: string }) {
  const versions = useCommandQuery<CatalogVersionSummary[]>(LIST_METHOD_VERSIONS, {
    input: { methodId },
  });
  return (
    <QueryContent query={versions} what="method versions">
      {(rows) => (
        <Select
          layout="inline"
          label="Version (approved)"
          name="version"
          options={rows
            .filter((version) => version.status === "approved")
            .map((version) => ({
              id: version.id,
              label: `Version ${version.version}${version.effectiveTo ? " — existing orders" : " — current"}`,
            }))}
          value={value}
          onChange={onChange}
        />
      )}
    </QueryContent>
  );
}

function Worklist({ query }: { query: ReturnType<typeof useCommandQuery<WorklistEntry[]>> }) {
  return (
    <QueryContent
      query={query}
      what="worklist"
      empty={{
        when: (rows) => rows.length === 0,
        title: "No containers waiting",
        description: "No accepted container needs this method version.",
      }}
    >
      {(rows) => (
        <DataGrid
          label="Waiting containers"
          columns={COLUMNS}
          rows={rows}
          getRowId={(row) => row.containerId}
          fill
        />
      )}
    </QueryContent>
  );
}
