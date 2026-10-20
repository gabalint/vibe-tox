import { useState } from "react";
import { useLocation } from "react-router";
import { Button } from "@/components/Button";
import { EmptyState } from "@/components/EmptyState";
import { FindCount, FindField, findRows, NoResults } from "@/components/FindField";
import { Select } from "@/components/Select";
import { Toolbar } from "@/components/Toolbar";
import { useFindParam } from "@/lib/findParam";
import { useOpenRecordPage } from "@/lib/recordSequence";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

const pad = (value: number) => String(value).padStart(2, "0");

const ANALYTES = Array.from({ length: 24 }, (_, index) => ({
  code: `SYN-AN-${pad(index + 1)}`,
  name: `Synthetic analyte ${index % 2 === 0 ? "alpha" : "beta"} ${index + 1}`,
}));

/** Rows shown in this demo; a screen shows its rows in a `DataGrid`. */
const SHOWN = 6;

/**
 * A directory's find field in `?q=`, its count, the no-results state, and opening a row's
 * record page with the list's order (see "Record page header and tabs").
 */
export function FindSection() {
  const { pathname } = useLocation();
  const [query, setQuery] = useFindParam();
  const [group, setGroup] = useState<string | null>("all");
  const open = useOpenRecordPage((code) => `${pathname}?record=${code}`, "Synthetic analytes");
  const rows = findRows(ANALYTES, query, (row) => [row.code, row.name]).filter(
    (row) => group === "all" || row.name.includes(group ?? ""),
  );
  const ids = rows.map((row) => row.code);
  return (
    <GallerySection title="Find in a directory">
      <Toolbar
        variant="collection"
        count={
          <FindCount
            shown={rows.length}
            total={ANALYTES.length}
            noun="synthetic analytes"
            one="synthetic analyte"
          />
        }
        moreFilters={{
          content: (
            <Select
              layout="inline"
              label="Group"
              name="group"
              value={group}
              onChange={setGroup}
              options={[
                { id: "all", label: "All groups" },
                { id: "alpha", label: "Alpha" },
                { id: "beta", label: "Beta" },
              ]}
            />
          ),
          applied: group !== "all" ? `Group: ${group}` : undefined,
        }}
      >
        <FindField
          label="Find synthetic analytes"
          placeholder="Code or name"
          value={query}
          onChange={setQuery}
        />
      </Toolbar>
      {rows.length === 0 ? (
        <NoResults noun="synthetic analytes" query={query} onClear={() => setQuery("")} />
      ) : (
        <div className={styles.row}>
          {rows.slice(0, SHOWN).map((row) => (
            <Button key={row.code} onPress={() => open(row.code, ids)}>
              {`Open ${row.code}`}
            </Button>
          ))}
        </div>
      )}
      <EmptyState
        title="No synthetic analytes yet"
        description="The empty state, distinct from no results."
        action={<Button variant="primary">Add synthetic analyte</Button>}
      />
    </GallerySection>
  );
}
