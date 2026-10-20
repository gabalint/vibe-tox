import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { Button } from "@/components/Button";
import { DataGrid } from "@/components/DataGrid";
import { EmptyState } from "@/components/EmptyState";
import { FindField } from "@/components/FindField";
import { Page, PageHeader } from "@/components/Page";
import { QueryContent } from "@/components/QueryContent";
import { RefreshControl } from "@/components/RefreshControl";
import { Tabs } from "@/components/Tabs";
import type { LabelPrinterView } from "@/generated/LabelPrinterView";
import type { StorageLocationView } from "@/generated/StorageLocationView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
import { LocationDialog } from "./LocationDialog";
import { PrinterDialog } from "./PrinterDialog";

export function MaterialConfigurationPage() {
  const [params] = useSearchParams();
  const accession = useHasCapability("accession");
  const quality = useHasCapability("manage_quality");
  const tab = params.get("tab") === "locations" ? "locations" : "printers";
  return (
    <Page>
      <PageHeader
        title="Printers and storage"
        description="Configure local label destinations and named material storage locations."
        actions={(accession || quality) && <Link to="/specimens/storage">Storage excursions</Link>}
      />
      <Tabs
        label="Material configuration"
        selectedId={tab}
        tabs={[
          { id: "printers", label: "Label printers", href: "/setup/material?tab=printers" },
          { id: "locations", label: "Storage locations", href: "/setup/material?tab=locations" },
        ]}
      >
        {tab === "printers" ? <PrinterDirectory /> : <LocationDirectory />}
      </Tabs>
    </Page>
  );
}
function useDirectoryFind() {
  const [params, setParams] = useSearchParams();
  return {
    find: params.get("q") ?? "",
    setFind: (q: string) =>
      setParams((old) => {
        const next = new URLSearchParams(old);
        if (q) next.set("q", q);
        else next.delete("q");
        return next;
      }),
  };
}
function PrinterDirectory() {
  const query = useCommandQuery<LabelPrinterView[]>("list_printers");
  const canEdit = useHasCapability("manage_configuration");
  const [editing, setEditing] = useState<LabelPrinterView | "new" | null>(null);
  const { find, setFind } = useDirectoryFind();
  return (
    <>
      <RefreshControl query={query} />
      {canEdit && <Button onPress={() => setEditing("new")}>Add label printer</Button>}
      <QueryContent query={query} what="label printers">
        {(all) => {
          const rows = all.filter((r) =>
            `${r.code} ${r.name} ${r.ipAddress ?? ""} ${r.queue ?? ""}`
              .toLowerCase()
              .includes(find.toLowerCase()),
          );
          return (
            <>
              <FindField
                label="Find label printers"
                value={find}
                onChange={setFind}
                count={{ shown: rows.length, total: all.length, noun: "printers", one: "printer" }}
              />
              {all.length === 0 ? (
                <EmptyState
                  title="No label printer is configured"
                  description="Add the laboratory printer destination, then return to the accession to print and check a synthetic label."
                />
              ) : (
                <DataGrid
                  label="Label printers"
                  rows={rows}
                  getRowId={(r) => r.printerId}
                  onRowAction={canEdit ? (r) => setEditing(r) : undefined}
                  columns={[
                    { id: "code", header: "Code", cell: (r) => r.code },
                    { id: "name", header: "Name", cell: (r) => r.name },
                    {
                      id: "destination",
                      header: "Destination",
                      cell: (r) =>
                        r.transport === "tcp" ? `${r.ipAddress}:${r.port}` : `CUPS ${r.queue}`,
                    },
                    {
                      id: "active",
                      header: "State",
                      cell: (r) => (r.active ? "Active" : "Inactive"),
                    },
                  ]}
                />
              )}
            </>
          );
        }}
      </QueryContent>
      {editing && (
        <PrinterDialog
          printer={editing === "new" ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </>
  );
}
function LocationDirectory() {
  const query = useCommandQuery<StorageLocationView[]>("list_locations");
  const canEdit = useHasCapability("manage_configuration");
  const [editing, setEditing] = useState<StorageLocationView | "new" | null>(null);
  const { find, setFind } = useDirectoryFind();
  return (
    <>
      <RefreshControl query={query} />
      {canEdit && <Button onPress={() => setEditing("new")}>Add storage location</Button>}
      <QueryContent query={query} what="storage locations">
        {(all) => {
          const rows = all.filter((r) =>
            `${r.code} ${r.name}`.toLowerCase().includes(find.toLowerCase()),
          );
          return (
            <>
              <FindField
                label="Find storage locations"
                value={find}
                onChange={setFind}
                count={{
                  shown: rows.length,
                  total: all.length,
                  noun: "locations",
                  one: "location",
                }}
              />
              {all.length === 0 ? (
                <EmptyState
                  title="No storage location is configured"
                  description="Add named locations before recording material moves or excursions."
                />
              ) : (
                <DataGrid
                  label="Storage locations"
                  rows={rows}
                  getRowId={(r) => r.locationId}
                  onRowAction={canEdit ? (r) => setEditing(r) : undefined}
                  columns={[
                    { id: "code", header: "Code", cell: (r) => r.code },
                    { id: "name", header: "Name", cell: (r) => r.name },
                    {
                      id: "condition",
                      header: "Storage condition",
                      cell: (r) => r.storageCondition ?? "Not recorded",
                    },
                    {
                      id: "active",
                      header: "State",
                      cell: (r) => (r.active ? "Active" : "Inactive"),
                    },
                  ]}
                />
              )}
            </>
          );
        }}
      </QueryContent>
      {editing && (
        <LocationDialog
          location={editing === "new" ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </>
  );
}
