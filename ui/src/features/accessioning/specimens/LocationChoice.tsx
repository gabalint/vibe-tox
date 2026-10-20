import { Link } from "react-router";
import { RecordPickerInput } from "@/components/FormFields";
import { QueryContent } from "@/components/QueryContent";
import type { StorageLocationView } from "@/generated/StorageLocationView";
import { useCommandQuery, useHasCapability } from "@/lib/commands";
export function LocationChoice() {
  const canConfigure = useHasCapability("manage_configuration");
  const query = useCommandQuery<StorageLocationView[]>("list_locations");
  return (
    <QueryContent query={query} what="storage locations">
      {(rows) => (
        <>
          <RecordPickerInput
            name="locationId"
            label="Storage location"
            options={rows
              .filter((r) => r.active)
              .map((r) => ({ id: r.locationId, label: `${r.code} — ${r.name}` }))}
            isRequired
          />
          {!rows.some((r) => r.active) && (
            <p>
              No active storage locations.{" "}
              {canConfigure ? (
                <Link to="/setup/material?tab=locations">Configure storage locations</Link>
              ) : (
                "An administrator must configure the required storage location."
              )}
            </p>
          )}
        </>
      )}
    </QueryContent>
  );
}
